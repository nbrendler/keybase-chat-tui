use std::process::{Command, Stdio};

use serde::{Serialize, Deserialize};
use serde_json::Result;

#[derive(Serialize, Deserialize)]
pub enum KeybaseMethod {
    list
    }

#[derive(Serialize, Deserialize)]
pub struct KeybaseCommand {
    pub method: KeybaseMethod
}

pub fn keybase_exec(command: KeybaseCommand) -> Result<String> {
    let mut child = Command::new("keybase").arg("chat").arg("api")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn Keybase");

    {
        let stdin = child.stdin.as_mut().expect("Failed to get child stdin");
        serde_json::to_writer(stdin, &command)?;
    }

    let output = child.wait_with_output().expect("No Keybase output");
    Ok(String::from_utf8(output.stdout).expect("Failed UTF8 conversion"))
}
