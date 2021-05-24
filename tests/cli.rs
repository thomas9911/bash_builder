use std::process::{Command, Output};

const BINARY: &'static str = "./target/debug/bash_bundler";

const CONFIG_PATH: &'static str = "./test_config.toml";

fn call_binary<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(BINARY)
        .args(args)
        .output()
        .expect("failed to execute process")
}

fn call_binary_to_string<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let out = call_binary(args);

    String::from_utf8(out.stdout).unwrap()
}

fn call_shell(shell_script: &str) -> Output {
    Command::new("sh")
        .arg("-c")
        .arg(shell_script)
        .output()
        .expect("failed to execute process")
}

#[test]
fn comment() {
    let out = call_binary_to_string(&["tests/one.sh"]);

    let expected = r#"yell() {
    echo "$1 !!!" | tr '[:lower:]' '[:upper:]'
}
print() {
    echo "$1"
}
yell "hallo"
print "hallo"
"#;

    assert_eq!(expected, out);

    // check if script is valid shell script
    let out = String::from_utf8(call_shell(&out).stdout).unwrap();
    let expected = "HALLO !!!\nhallo\n";
    assert_eq!(expected, out);
}

#[test]
fn comment_disabled() {
    let out = call_binary_to_string(&["tests/one.sh", "--disable-comment"]);

    let expected = r#"# import ./bash/one_utils.sh
# import ./bash/one_more_utils.sh
yell "hallo"
print "hallo"
"#;

    assert_eq!(expected, out)
}

#[test]
fn config() {
    let out = call_binary_to_string(&["--config", CONFIG_PATH]);

    let expected = r#"yell() {
    echo "$1 !!!" | tr '[:lower:]' '[:upper:]'
}
print() {
    echo "$1"
}

this_is_from_sourced_file() {
    yell "$1 !!!!!!"
}

yell "hallo"
print "hallo"
"#;

    assert_eq!(expected, out);

    // check if script is valid shell script
    let out = String::from_utf8(call_shell(&out).stdout).unwrap();
    let expected = "HALLO !!!\nhallo\n";
    assert_eq!(expected, out);
}

#[test]
fn source() {
    let out = call_binary_to_string(&["tests/source.sh", "--enable-source"]);

    let expected = r#"yell() {
    echo "$1 !!!" | tr '[:lower:]' '[:upper:]'
}
print() {
    echo "$1"
}

this_is_from_sourced_file() {
    yell "$1 !!!!!!"
}

yell "hallo"
print "hallo"
"#;
    assert_eq!(expected, out);

    // check if script is valid shell script
    let out = String::from_utf8(call_shell(&out).stdout).unwrap();
    let expected = "HALLO !!!\nhallo\n";
    assert_eq!(expected, out);
}

#[test]
fn source_disabled() {
    let out = call_binary_to_string(&["tests/source.sh"]);

    let expected = r#"source ./bash/source_utils.sh

yell "hallo"
print "hallo"
"#;

    assert_eq!(expected, out)
}

#[test]
fn file_or_config_required() {
    let args: &[&str] = &[];
    let out = call_binary(args);

    assert!(!out.status.success());
}
