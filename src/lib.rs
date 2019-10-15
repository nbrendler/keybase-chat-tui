use std::process::Command;

pub fn test() {
    println!("test");
}

struct KeybaseOptions;

fn keybase_exec<I, S>(args: I, options: KeybaseOptions) where I: IntoIterator<Item=S>, S: AsRef<OsStr>{
    let child = Command::new("keybase")
        .args(args)
        .spawn
        
}
