extern crate structopt;

use structopt::StructOpt;
use std::env;
use dove::{cmd::*};
use std::process::exit;
use anyhow::Error;
use dove::cmd::clean::Clean;
use dove::cmd::init::Init;
use dove::cmd::new::New;
use dove::cmd::metadata::Metadata;
use dove::cmd::fetch::Fetch;
use lang::compiler::ConstPool;
use dove::cmd::build::Build;
use dove::cmd::test::Test;
use dove::cmd::run::Run;

#[derive(StructOpt, Debug)]
#[structopt(name = "Move compiler.", version = git_hash::crate_version_with_git_hash_short ! ())]
enum Opt {
    #[structopt(about = "Init directory as move project")]
    Init {
        #[structopt(flatten)]
        cmd: Init,
    },
    #[structopt(about = "Create a new move project")]
    New {
        #[structopt(flatten)]
        cmd: New,
    },
    #[structopt(about = "Print metadata")]
    Metadata {
        #[structopt(flatten)]
        cmd: Metadata,
    },
    #[structopt(about = "Fetch project dependencies")]
    Fetch {
        #[structopt(flatten)]
        cmd: Fetch,
    },
    #[structopt(about = "Build project")]
    Build {
        #[structopt(flatten)]
        cmd: Build,
    },
    #[structopt(about = "Remove the target directory")]
    Clean {
        #[structopt(flatten)]
        cmd: Clean,
    },
    #[structopt(about = "Run move tests")]
    Test {
        #[structopt(flatten)]
        cmd: Test,
    },
    #[structopt(about = "Run move tests")]
    Run {
        #[structopt(flatten)]
        cmd: Run,
    },
}

fn main() {
    let matches = Opt::from_args();

    let _pool = ConstPool::new();
    handle_error(match matches {
        Opt::Clean { cmd } => cmd.execute(),
        Opt::New { cmd } => cmd.execute(),
        Opt::Init { cmd } => cmd.execute(),
        Opt::Metadata { cmd } => cmd.execute(),
        Opt::Fetch { cmd } => cmd.execute(),
        Opt::Build { cmd } => cmd.execute(),
        Opt::Test { cmd } => cmd.execute(),
        Opt::Run { cmd } => cmd.execute(),
    });
}

fn handle_error<T>(res: Result<T, Error>) -> T {
    match res {
        Ok(t) => t,
        Err(err) => {
            println!("error: {:?}.", err);
            exit(1);
        }
    }
}