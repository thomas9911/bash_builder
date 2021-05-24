use serde_derive::Deserialize;
use std::ffi::OsStr;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

const CIRCULAR_CUT_OFF: usize = 512;

#[derive(Debug, Deserialize)]
pub struct Config {
    builder: Args,
}

/// Collects/bundles bash files into one file.
///
/// By default uses the saver `# import ./filename.sh` syntax to include other bash files.
/// But can be set to use the already existing `source ./filename.sh` syntax.
///
/// There is a difference between the `import` and `source` import statements.
/// The `import` is relative to the current file, but the `source` is relative from the base/root file.
///
/// for instance:
/// your root file is in `src/my_project.sh` that looks like:
///
/// ```sh
/// # import ./utils/utils.sh
///
/// my_func "hallo"
/// ```
///
/// and utils.sh looks like:
///
/// ```sh
/// # import ./other.sh # other contains the my_func
/// ```
/// this will import from file `./src/utils/other.sh`
///
/// With the source it is relative from the root file so like:
///
/// ```sh
/// source ./utils/utils.sh
///
/// my_func "hallo"
/// ```
///
/// and `utils.sh` looks like:
///
/// ```sh
/// source ./utils/other.sh # other contains the my_func
/// ```
///
/// This is done so that files containing the `source` can just be used in normal bash.
/// ```sh
/// cd src
/// ./my_project.sh
/// ```
///
/// Configs can be used to override/save arguments. Config should look like:
///
/// ```toml
///
/// [builder]
/// replace_source = true
/// replace_comment = false
/// root_path = "./tests/source.sh"
/// ```
///
#[derive(Debug, StructOpt, Deserialize)]
#[structopt(verbatim_doc_comment)]
#[serde(default)]
pub struct Args {
    /// starting or `main` bash file
    #[structopt(required_unless("config"), parse(try_from_str = existing_path))]
    root_path: Option<PathBuf>,
    #[serde(skip)]
    /// path to your toml config
    #[structopt(short, long, parse(try_from_str = existing_path))]
    config: Option<PathBuf>,
    /// enable the 'source ./file.sh` syntax
    #[structopt(long = "enable-source")]
    replace_source: bool,
    /// disable the '# import ./file.sh` syntax
    #[structopt(long = "disable-comment", parse(from_flag = std::ops::Not::not))]
    replace_comment: bool,
}

impl Default for Args {
    fn default() -> Args {
        Args {
            root_path: None,
            config: None,
            replace_comment: true,
            replace_source: false,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Toml(toml::de::Error),
    Circular,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{}", err),
            Error::Toml(err) => write!(f, "{}", err),
            Error::Circular => write!(f, "Circular import found"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Error {
        Error::Toml(err)
    }
}

fn main() -> Result<(), String> {
    match inner_main() {
        Ok(output) => Ok(println!("{}", output)),
        Err(e) => Err(e.to_string()),
    }
}

fn inner_main() -> Result<String, Error> {
    let mut args = Args::from_args();
    if let Some(config) = args.config {
        let configs = std::fs::read(config)?;
        let loaded: Config = toml::from_slice(&configs)?;
        args = loaded.builder;
    }

    if let Some(x) = args.root_path.clone() {
        let bash_file = BashFile::resolve(x, &args)?;

        return Ok(bash_file.to_string());
    }

    Err(Error::Io(io::ErrorKind::NotFound.into()))
}

fn existing_path(path: &str) -> Result<PathBuf, Error> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(Error::Io(io::ErrorKind::NotFound.into()));
    }

    Ok(path)
}

#[derive(Debug)]
pub enum ImportStyle {
    Comment,
    Source,
}

#[derive(Debug)]
pub struct ImportStatement {
    line_number: usize,
    line: String,
    text: String,
    path: PathBuf,
    style: ImportStyle,
    resolved: Option<BashFile>,
}

#[derive(Debug, Default)]
/// container for a bash file
pub struct BashFile {
    path: PathBuf,
    contents: Option<String>,
    dependents: Vec<ImportStatement>,
    nested: usize,
}

impl std::fmt::Display for BashFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.contents {
            None => write!(f, ""),
            Some(contents) => write!(f, "{}", contents),
        }
    }
}

impl BashFile {
    /// loads, imports and resolves the file
    pub fn resolve(path: PathBuf, config: &Args) -> Result<Self, Error> {
        BashFile::new(path)
            .load()?
            .load_dependents(config)?
            .resolve_dependents(config)
    }

    /// create a new BashFile struct
    pub fn new(path: PathBuf) -> Self {
        BashFile {
            path,
            ..Default::default()
        }
    }

    /// load the file from the path
    pub fn load(mut self) -> Result<Self, Error> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        let mut contents = String::new();
        reader.read_to_string(&mut contents)?;

        self.contents = Some(contents);
        Ok(self)
    }

    /// interate over the lines in the file
    pub fn lines<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self.contents {
            None => Box::new(std::iter::empty()),
            Some(ref input) => Box::new(input.lines()),
        }
    }

    /// interate over the imports found in the file
    pub fn imports<'a>(
        &'a self,
        config: &'a Args,
    ) -> Box<dyn Iterator<Item = ImportStatement> + 'a> {
        let path = PathBuf::from(self.path.parent().unwrap());
        Box::new(
            self.lines()
                .enumerate()
                .filter_map(move |(index, x)| Self::to_import(x, index, path.clone(), config)),
        )
    }

    /// load the imports found in the file
    pub fn load_dependents(mut self, config: &Args) -> Result<Self, Error> {
        let mut deps = Vec::new();

        for mut import in self.imports(config) {
            let file = BashFile::new(import.path.clone())
                .load()?
                .inner_load_dependents(self.nested + 1, config)?;
            import.resolved = Some(file);
            deps.push(import)
        }

        self.dependents = deps;
        Ok(self)
    }

    fn inner_load_dependents(mut self, nested: usize, config: &Args) -> Result<Self, Error> {
        if nested > CIRCULAR_CUT_OFF {
            return Err(Error::Circular);
        }
        self.nested = nested;

        self.load_dependents(config)
    }

    /// replace the imports found in the file with the importered files
    pub fn resolve_dependents(mut self, config: &Args) -> Result<Self, Error> {
        let mut lines: Vec<String> = self.lines().map(String::from).collect();
        for import in self.dependents {
            if let Some(mut dep) = import.resolved {
                dep.nested += 1;
                let loaded_dep = dep.load_dependents(config)?.resolve_dependents(config)?;
                // let line = &import.line;
                // if let Some(index) = lines.iter().position(|x| x.starts_with(line)) {
                //     println!("{} => {}", index, import.line_number);
                //     lines.remove(index);
                //     lines.insert(index, loaded_dep.contents.unwrap_or(String::new()));
                // };
                lines.remove(import.line_number);
                lines.insert(
                    import.line_number,
                    loaded_dep.contents.unwrap_or(String::new()),
                );
            }
        }
        self.contents = Some(lines.join("\n"));
        self.dependents = Vec::new();
        Ok(self)
    }

    fn to_import(
        input: &str,
        line_number: usize,
        path: PathBuf,
        config: &Args,
    ) -> Option<ImportStatement> {
        // is comment style
        if config.replace_comment {
            if let Some(x) = input.strip_prefix("# import ") {
                if let Some((line_part, resolve_path)) = Self::to_valid_bash_file(path, x) {
                    return Some(ImportStatement {
                        line: String::from(input),
                        path: resolve_path,
                        text: String::from(line_part),
                        style: ImportStyle::Comment,
                        resolved: None,
                        line_number,
                    });
                }
            }
        }

        if config.replace_source {
            if let Some(x) = input.strip_prefix("source ") {
                let root_path = config
                    .root_path
                    .clone()
                    .expect("root path should be checked already")
                    .parent()
                    .expect("file can never be root dir")
                    .into();
                if let Some((line_part, resolve_path)) = Self::to_valid_bash_file(root_path, x) {
                    return Some(ImportStatement {
                        line: String::from(input),
                        path: resolve_path,
                        text: String::from(line_part),
                        style: ImportStyle::Source,
                        resolved: None,
                        line_number,
                    });
                }
            }
        }

        None
    }

    fn to_valid_bash_file(mut path: PathBuf, to_test_file: &str) -> Option<(&str, PathBuf)> {
        path.push(Path::new(to_test_file));

        if path.exists() && path.extension() == Some(OsStr::new("sh")) {
            return Some((to_test_file, path));
        }

        None
    }
}

#[test]
fn resolving_one_level() {
    let file = BashFile::resolve("./tests/one.sh".into(), &Args::default()).unwrap();

    let expected = r#"yell() {
    echo "$1 !!!" | tr '[:lower:]' '[:upper:]'
}
print() {
    echo "$1"
}
yell "hallo"
print "hallo""#;

    assert_eq!(expected, file.to_string())
}

#[test]
fn resolving_two_level() {
    let file = BashFile::resolve("./tests/two.sh".into(), &Args::default()).unwrap();

    let expected = r#"yell() {
    echo "$1 !!!" | tr '[:lower:]' '[:upper:]'
}


super_yell() {
    yell "$1 !!!!!!"
}
print() {
    echo "$1"
}
yell "hallo"
print "hallo"
super_yell "hallo""#;

    assert_eq!(expected, file.to_string())
}

#[test]
fn resolving_circular() {
    let file = BashFile::resolve("./tests/circular.sh".into(), &Args::default())
        .unwrap_err()
        .to_string();
    let expected = Error::Circular.to_string();
    assert_eq!(expected, file)
}

#[test]
fn resolving_source() {
    let mut args = Args::default();
    args.root_path = Some("./tests/source.sh".into());
    args.replace_source = true;
    args.replace_comment = false;

    let file = BashFile::resolve("./tests/source.sh".into(), &args).unwrap();

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
print "hallo""#;

    assert_eq!(expected, file.to_string())
}
