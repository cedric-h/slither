use crate::agent::{Agent, MioMapType};
use crate::intrinsics::promise::new_promise_capability;
use crate::value::{new_builtin_function, new_error, Value};
use crate::vm::ExecutionContext;
use mio::{PollOpt, Ready, Registration, Token};
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref RESPONSES: Mutex<HashMap<Token, FsResponse>> = Mutex::new(HashMap::new());
}

pub enum FsResponse {
    Read(String),
    Success,
    Error(String),
}

pub fn handle(agent: &Agent, token: Token, promise: Value) {
    let fsr = RESPONSES.lock().unwrap().remove(&token).unwrap();
    match fsr {
        FsResponse::Read(s) => {
            promise
                .get_slot("resolve")
                .call(agent, promise, vec![Value::String(s)])
                .unwrap();
        }
        FsResponse::Success => {
            promise
                .get_slot("resolve")
                .call(agent, promise, vec![])
                .unwrap();
        }
        FsResponse::Error(s) => {
            promise
                .get_slot("reject")
                .call(agent, promise, vec![new_error(s.as_str())])
                .unwrap();
        }
    }
}

fn read_file(agent: &Agent, _c: &ExecutionContext, args: Vec<Value>) -> Result<Value, Value> {
    if let Some(Value::String(filename)) = args.get(0) {
        let promise = new_promise_capability(agent, agent.intrinsics.promise.clone())?;

        let (registration, set_readiness) = Registration::new2();
        let token = Token(agent.mio_map.borrow().len());

        agent
            .mio
            .register(&registration, token, Ready::readable(), PollOpt::edge())
            .unwrap();
        agent
            .mio_map
            .borrow_mut()
            .insert(token, MioMapType::FS(registration, promise.clone()));

        let filename = filename.to_string();
        agent
            .pool
            .execute(move || match std::fs::read_to_string(filename) {
                Ok(s) => {
                    RESPONSES.lock().unwrap().insert(token, FsResponse::Read(s));
                    set_readiness.set_readiness(Ready::readable()).unwrap();
                }
                Err(e) => {
                    RESPONSES
                        .lock()
                        .unwrap()
                        .insert(token, FsResponse::Error(format!("{}", e)));
                    set_readiness.set_readiness(Ready::readable()).unwrap();
                }
            });

        Ok(promise)
    } else {
        Err(new_error("filename must be a string"))
    }
}

fn write_file(agent: &Agent, _c: &ExecutionContext, args: Vec<Value>) -> Result<Value, Value> {
    if let Some(Value::String(filename)) = args.get(0) {
        if let Some(Value::String(contents)) = args.get(1) {
            let promise = new_promise_capability(agent, agent.intrinsics.promise.clone())?;

            let (registration, set_readiness) = Registration::new2();
            let token = Token(agent.mio_map.borrow().len());

            agent
                .mio
                .register(&registration, token, Ready::readable(), PollOpt::edge())
                .unwrap();
            agent
                .mio_map
                .borrow_mut()
                .insert(token, MioMapType::FS(registration, promise.clone()));

            let filename = filename.to_string();
            let contents = contents.to_string();
            agent
                .pool
                .execute(move || match std::fs::write(filename, contents) {
                    Ok(()) => {
                        RESPONSES.lock().unwrap().insert(token, FsResponse::Success);
                        set_readiness.set_readiness(Ready::readable()).unwrap();
                    }
                    Err(e) => {
                        RESPONSES
                            .lock()
                            .unwrap()
                            .insert(token, FsResponse::Error(format!("{}", e)));
                        set_readiness.set_readiness(Ready::readable()).unwrap();
                    }
                });

            Ok(promise)
        } else {
            Err(new_error("contents must be a string"))
        }
    } else {
        Err(new_error("filename must be a string"))
    }
}

fn remove_file(agent: &Agent, _c: &ExecutionContext, args: Vec<Value>) -> Result<Value, Value> {
    if let Some(Value::String(filename)) = args.get(0) {
        let promise = new_promise_capability(agent, agent.intrinsics.promise.clone())?;

        let (registration, set_readiness) = Registration::new2();
        let token = Token(agent.mio_map.borrow().len());

        agent
            .mio
            .register(&registration, token, Ready::readable(), PollOpt::edge())
            .unwrap();
        agent
            .mio_map
            .borrow_mut()
            .insert(token, MioMapType::FS(registration, promise.clone()));

        let filename = filename.to_string();
        agent
            .pool
            .execute(move || match std::fs::remove_file(filename) {
                Ok(()) => {
                    RESPONSES.lock().unwrap().insert(token, FsResponse::Success);
                    set_readiness.set_readiness(Ready::readable()).unwrap();
                }
                Err(e) => {
                    RESPONSES
                        .lock()
                        .unwrap()
                        .insert(token, FsResponse::Error(format!("{}", e)));
                    set_readiness.set_readiness(Ready::readable()).unwrap();
                }
            });

        Ok(promise)
    } else {
        Err(new_error("filename must be a string"))
    }
}

pub fn create(agent: &Agent) -> HashMap<String, Value> {
    let mut module = HashMap::new();

    macro_rules! method {
        ($name:expr, $fn:ident) => {
            module.insert($name.to_string(), new_builtin_function(agent, $fn));
        };
    }
    method!("readFile", read_file);
    method!("writeFile", write_file);
    method!("removeFile", remove_file);
    // stat
    // copy
    // move
    // createSymbolicLink
    // exists
    // watch
    // createDirectory
    // removeDirectory
    // readDirectory

    module
}
