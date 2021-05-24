# bash_bundler

Collects/bundles bash files into one file.

By default uses the saver `# import ./filename.sh` syntax to include other bash files.
But can be set to use the already existing `source ./filename.sh` syntax.

There is a difference between the `import` and `source` import statements.
The `import` is relative to the current file, but the `source` is relative from the base/root file.

for instance:
your root file is in `src/my_project.sh` that looks like:

```sh
# import ./utils/utils.sh

my_func "hallo"
```

and utils.sh looks like:

```sh
# import ./other.sh # other contains the my_func
```

this will import from file `./src/utils/other.sh

With the source it is relative from the root file so like:

```sh
source ./utils/utils.sh

my_func "hallo"
```

and utils.sh looks like:

```sh
source ./utils/other.sh # other contains the my_func
```

This is done so that files containing the `source` can just be used in normal bash.

```sh
cd src
./my_project.sh
```

Configs can be used to override/save arguments. Config should look like:

```toml

[builder]
replace_source = true
replace_comment = false
root_path = "./tests/source.sh"
```

```text
USAGE:
bash_bundler.exe [FLAGS] [OPTIONS] <root-path>

FLAGS:
-h, --help  
 Prints help information

        --disable-comment
            disable the '# import ./file.sh` syntax

        --enable-source
            enable the 'source ./file.sh` syntax

    -V, --version
            Prints version information

OPTIONS:
-c, --config <config>  
 path to your toml config

ARGS:
<root-path>  
 starting or `main` bash file
```
