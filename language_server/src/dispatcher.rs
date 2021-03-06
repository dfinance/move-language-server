use std::panic;

use anyhow::Result;
use crossbeam_channel::Sender;
use lsp_server::{ErrorCode, Message, Request, RequestId, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use threadpool::ThreadPool;

use crate::global_state::{GlobalState, GlobalStateSnapshot};
use crate::main_loop::{on_task, LspError, ResponseEvent};
use crate::req;

fn result_to_task<R>(id: RequestId, result: Result<R::Result>) -> ResponseEvent
where
    R: req::Request + 'static,
    R::Params: DeserializeOwned + 'static,
    R::Result: Serialize + 'static,
{
    let response = match result {
        Ok(resp) => Response::new_ok(id, &resp),
        Err(e) => match e.downcast::<LspError>() {
            Ok(lsp_error) => {
                if lsp_error.code == LspError::UNKNOWN_FILE {
                    // Work-around for https://github.com/rust-analyzer/rust-analyzer/issues/1521
                    Response::new_ok(id, ())
                } else {
                    Response::new_err(id, lsp_error.code, lsp_error.message)
                }
            }
            Err(e) => Response::new_err(id, ErrorCode::InternalError as i32, e.to_string()),
        },
    };
    ResponseEvent::Respond(response)
}

pub struct PoolDispatcher<'a> {
    // will be None after first matched on_* method
    req: Option<Request>,
    pool: &'a ThreadPool,
    global_state: &'a mut GlobalState,
    msg_sender: &'a Sender<Message>,
    task_sender: &'a Sender<ResponseEvent>,
}

impl<'a> PoolDispatcher<'a> {
    pub fn new(
        req: Request,
        pool: &'a ThreadPool,
        global_state: &'a mut GlobalState,
        msg_sender: &'a Sender<Message>,
        task_sender: &'a Sender<ResponseEvent>,
    ) -> PoolDispatcher<'a> {
        PoolDispatcher {
            req: Some(req),
            pool,
            global_state,
            msg_sender,
            task_sender,
        }
    }
    /// Dispatches the request onto the current thread
    pub fn on_sync<R>(
        &mut self,
        f: fn(&mut GlobalState, R::Params) -> Result<R::Result>,
    ) -> Result<&mut Self>
    where
        R: req::Request + 'static,
        R::Params: DeserializeOwned + panic::UnwindSafe + 'static,
        R::Result: Serialize + 'static,
    {
        let (id, params) = match self.parse::<R>() {
            Some(it) => it,
            None => {
                return Ok(self);
            }
        };
        let global_state = panic::AssertUnwindSafe(&mut *self.global_state);
        let task = panic::catch_unwind(move || {
            let result = f(global_state.0, params);
            result_to_task::<R>(id, result)
        })
        .map_err(|_| anyhow::anyhow!("sync task {:?} panicked", R::METHOD))?;

        on_task(task, self.msg_sender);
        Ok(self)
    }

    /// Dispatches the request onto thread pool
    pub fn on<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, R::Params) -> Result<R::Result>,
    ) -> Result<&mut Self>
    where
        R: req::Request + 'static,
        R::Params: DeserializeOwned + Send + 'static,
        R::Result: Serialize + 'static,
    {
        let (id, params) = match self.parse::<R>() {
            Some(it) => it,
            None => {
                return Ok(self);
            }
        };

        self.pool.execute({
            let state_snapshot = self.global_state.snapshot();
            let sender = self.task_sender.clone();
            move || {
                let result = f(state_snapshot, params);
                let task = result_to_task::<R>(id, result);
                sender.send(task).unwrap();
            }
        });

        Ok(self)
    }

    pub fn finish(&mut self) {
        match self.req.take() {
            None => (),
            Some(req) => {
                log::error!("unknown request: {:?}", req);
                let resp = Response::new_err(
                    req.id,
                    ErrorCode::MethodNotFound as i32,
                    "unknown request".to_string(),
                );
                self.msg_sender.send(resp.into()).unwrap();
            }
        }
    }

    fn parse<R>(&mut self) -> Option<(RequestId, R::Params)>
    where
        R: req::Request + 'static,
        R::Params: DeserializeOwned + 'static,
    {
        let req = self.req.take()?;
        let (id, params) = match req.extract::<R::Params>(R::METHOD) {
            Ok(it) => it,
            Err(req) => {
                self.req = Some(req);
                return None;
            }
        };
        Some((id, params))
    }
}
