use std::collections::HashMap;

use anyhow::{Context, Result};
use libra::move_core_types::account_address::AccountAddress;
use libra::move_core_types::identifier::Identifier;
use libra::move_core_types::language_storage::{ModuleId, StructTag, TypeTag};
use libra::move_core_types::vm_status::{StatusCode, VMStatus};
use libra::move_vm_runtime::data_cache::{RemoteCache, TransactionEffects};
use libra::move_vm_runtime::move_vm::MoveVM;
use libra::move_vm_types::gas_schedule::CostStrategy;
use libra::move_vm_types::values::Value;
use libra::vm::access::ModuleAccess;
use libra::vm::CompiledModule;
use libra::vm::errors::{Location, PartialVMError, PartialVMResult, VMResult};
use libra::vm::file_format::{CompiledScript, FunctionDefinitionIndex};

use crate::explain::{
    explain_effects, StepExecutionResult, explain_abort, explain_execution_failure,
    explain_type_error,
};
use crate::meta::ExecutionMeta;
use crate::oracles::{oracle_coins_module, time_metadata, coin_balance_metadata, block_metadata};
use libra::move_vm_runtime::logging::NoContextLog;
use crate::session::ConstsMap;

pub type SerializedTransactionEffects = Vec<((AccountAddress, StructTag), Option<Vec<u8>>)>;

#[derive(Debug, Default, Clone)]
pub struct FakeRemoteCache {
    modules: HashMap<ModuleId, Vec<u8>>,
    resources: HashMap<(AccountAddress, StructTag), Vec<u8>>,
}

impl FakeRemoteCache {
    pub fn new(compiled_modules: Vec<CompiledModule>) -> Result<Self> {
        let mut modules = HashMap::with_capacity(compiled_modules.len());
        for module in compiled_modules {
            let mut module_bytes = vec![];
            module
                .serialize(&mut module_bytes)
                .context("Module serialization error")?;
            modules.insert(module.self_id(), module_bytes);
        }
        let resources = HashMap::new();
        Ok(FakeRemoteCache { modules, resources })
    }

    /// Read the resource bytes stored on-disk at `addr`/`tag`
    pub fn get_resource_bytes(&self, addr: AccountAddress, tag: StructTag) -> Option<Vec<u8>> {
        self.resources.get(&(addr, tag)).map(|r| r.to_owned())
    }

    /// Read the resource bytes stored on-disk at `addr`/`tag`
    fn get_module_bytes(&self, module_id: &ModuleId) -> Option<Vec<u8>> {
        self.modules.get(module_id).map(|r| r.to_owned())
    }

    /// Deserialize and return the module stored on-disk at `addr`/`module_id`
    pub fn get_compiled_module(&self, module_id: &ModuleId) -> Result<CompiledModule> {
        CompiledModule::deserialize(&self.get_module_bytes(module_id).unwrap())
            .map_err(|e| anyhow::anyhow!("Failure deserializing module {:?}: {:?}", module_id, e))
    }

    pub fn resolve_function(&self, module_id: &ModuleId, idx: u16) -> Result<Identifier> {
        let m = self.get_compiled_module(module_id).unwrap();
        Ok(m.identifier_at(
            m.function_handle_at(m.function_def_at(FunctionDefinitionIndex(idx)).function)
                .name,
        )
        .to_owned())
    }

    pub fn serialize_effects(
        &self,
        effects: TransactionEffects,
    ) -> (SerializedTransactionEffects, usize) {
        let mut resources_write_size = 0;
        let mut resources = vec![];
        for (addr, changes) in effects.resources {
            for (struct_tag, val) in changes {
                match val {
                    Some((layout, val)) => {
                        let serialized = val.simple_serialize(&layout).expect("Valid value.");
                        resources_write_size += serialized.len();
                        resources.push(((addr, struct_tag), Some(serialized)));
                    }
                    None => {
                        resources.push(((addr, struct_tag), None));
                    }
                }
            }
        }
        (resources, resources_write_size)
    }

    pub fn merge_effects(&mut self, serialized_effects: SerializedTransactionEffects) {
        for ((addr, struct_tag), val) in serialized_effects {
            match val {
                Some(val) => self.resources.insert((addr, struct_tag), val),
                None => self.resources.remove(&(addr, struct_tag)),
            };
        }
    }
}

impl RemoteCache for FakeRemoteCache {
    fn get_module(&self, module_id: &ModuleId) -> VMResult<Option<Vec<u8>>> {
        match self.modules.get(module_id) {
            None => {
                match self.get_module_bytes(module_id) {
                    Some(bytes) => Ok(Some(bytes)),
                    None => Err(PartialVMError::new(StatusCode::STORAGE_ERROR)
                        .finish(Location::Undefined)),
                }
            }
            m => Ok(m.cloned()),
        }
    }

    fn get_resource(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
    ) -> PartialVMResult<Option<Vec<u8>>> {
        let res = match self.resources.get(&(*address, struct_tag.clone())) {
            None => self.get_resource_bytes(*address, struct_tag.clone()),
            res => res.cloned(),
        };
        Ok(res)
    }
}

pub fn serialize_script(script: &CompiledScript) -> Result<Vec<u8>> {
    let mut serialized = vec![];
    script
        .serialize(&mut serialized)
        .context("Script serialization error")?;
    Ok(serialized)
}

fn execute_script_with_runtime_session<R: RemoteCache>(
    data_store: &R,
    script: Vec<u8>,
    args: Vec<Value>,
    ty_args: Vec<TypeTag>,
    senders: Vec<AccountAddress>,
    cost_strategy: &mut CostStrategy,
) -> VMResult<TransactionEffects> {
    let vm = MoveVM::new();
    let mut runtime_session = vm.new_session(data_store);

    runtime_session.execute_script(
        script,
        ty_args,
        args,
        senders,
        cost_strategy,
        &NoContextLog::new(),
    )?;
    runtime_session.finish()
}

pub fn execute_script(
    meta: ExecutionMeta,
    data_store: &mut FakeRemoteCache,
    script: CompiledScript,
    args: Vec<Value>,
    cost_strategy: &mut CostStrategy,
    consts_map: &ConstsMap,
) -> Result<StepExecutionResult> {
    let mut ds = data_store.clone();
    let ExecutionMeta {
        signers,
        accounts_balance,
        oracle_prices,
        current_time,
        aborts_with,
        status,
        block,
        dry_run,
    } = meta;
    if !oracle_prices.is_empty() {
        // check if module exists, and fail with MISSING_DEPENDENCY if not
        if ds.get_module(&oracle_coins_module()).is_err() {
            return Ok(StepExecutionResult::Error(
                "Cannot use `price:` comments: missing `0x1::Coins` module".to_string(),
            ));
        }
    }
    let std_addr = AccountAddress::from_hex_literal("0x1").expect("Standart address");

    if let Some(current_time) = current_time {
        ds.resources.insert(
            (std_addr, time_metadata()),
            libra::lcs::to_bytes(&current_time).unwrap(),
        );
    }
    let block_height = block.unwrap_or(100);
    ds.resources.insert(
        (std_addr, block_metadata()),
        libra::lcs::to_bytes(&block_height).unwrap(),
    );
    for (price_tag, val) in oracle_prices {
        ds.resources
            .insert((std_addr, price_tag), libra::lcs::to_bytes(&val).unwrap());
    }
    for (account, coin, val) in accounts_balance {
        ds.resources.insert(
            (account, coin_balance_metadata(&coin)),
            libra::lcs::to_bytes(&val).unwrap(),
        );
    }

    let res = execute_script_with_runtime_session(
        &ds,
        serialize_script(&script)?,
        args,
        vec![],
        signers.clone(),
        cost_strategy,
    );
    Ok(match res {
        Ok(effects) => {
            let mut explained = explain_effects(&effects, &ds)?;
            let (serialized_effects, effects_writeset_size) =
                data_store.serialize_effects(effects);
            explained.set_write_set_size(effects_writeset_size);
            if !dry_run {
                data_store.merge_effects(serialized_effects);
            }
            StepExecutionResult::Success(explained)
        }
        Err(vm_error) => {
            let vm_status = vm_error.into_vm_status();
            match vm_status {
                VMStatus::MoveAbort(_, code) => {
                    let error_message = explain_abort(vm_status, consts_map);
                    if let Some(abort_code) = aborts_with {
                        if code == abort_code {
                            StepExecutionResult::with_expected_error(error_message)
                        } else {
                            StepExecutionResult::with_error(error_message)
                        }
                    } else {
                        StepExecutionResult::with_error(error_message)
                    }
                }
                VMStatus::ExecutionFailure { status_code, .. } => {
                    let status_code = status_code as u64;
                    let error_message = explain_execution_failure(vm_status, data_store);
                    if let Some(expected_status_code) = status {
                        if status_code == expected_status_code {
                            StepExecutionResult::with_expected_error(error_message)
                        } else {
                            StepExecutionResult::with_error(error_message)
                        }
                    } else {
                        StepExecutionResult::with_error(error_message)
                    }
                }
                VMStatus::Error(StatusCode::TYPE_MISMATCH) => {
                    StepExecutionResult::with_error(explain_type_error(&script, &signers, &[]))
                }
                VMStatus::Error(status_code) => StepExecutionResult::with_error(format!(
                    "Execution failed with unexpected error {:?}",
                    status_code
                )),
                VMStatus::Executed => unreachable!(),
            }
        }
    })
}
