# 😁🦀 apikrab
CLI tools to manage your json api call in the terminal for fun only !


![apikrab](img/apikrab.png "apikrab")

## Philosophy

The goal of this project is to provide a simple tool to manage my json API calls in the
terminal. It is made to answer my needs which may be not the same for you.

This tool may be used in different ways:
 - to perform basic http call, providing a convenient CLI (but not as nice as HTTPie). Covers only essential features of
 an http tool.
 - performing project management with a collection of API calls (postman) and testing features


> [!WARNING]
> Tested on MacOs only

> [!IMPORTANT]
> I am a newbie in Rust programming... If you take a look to the code be indulgent !

## Installation

### Build from source
Install rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Clone the repo
```bash
git clone git@github.com:jerkos/apikrab.git
```
Build the project in release mode
```bash
cargo build --release
```
Add the binary to your path
```bash
export PATH=$PATH:/path/to/apikrab/target/release
```

### Download the binary

Grab the latest release from the [release page](https://github.com/jerkos/apikrab/releases)
for your platform (linux or darwin for the moment), and put in your path.

## Usage
```bash
ak --help
```
Works also for all subcommands, e.g.
```bash
ak project --help
```
![Help view](img/help.png "Help view")

## Making simple API call

Here some examples of how to make simple API calls:

```bash
ak run GET -u https://httpbin.org/anything
ak run GET -u https://httpbin.org/anything -q name:Marco -q age:18
ak run POST -u https://httpbin.org/anything -b name:Marco -b age:18
ak run POST -u https://httpbin.org/anything -b name:Marco -b age:18 --form-data
```

### Don't repeat yourself

The most annoying part for me is to rewrite / modify command line to test or adjust one API call.
To tackle this problem, you can save your call with a mindful name and replay it later as it is or
specifying new set of parameters.

For example:

```bash
ak run GET -u https://httpbin.org/anything -q name:Marco -q age:18 --save anything-marco
# then later
ak run action anything-marco # this will replay the API call
# or you can specify new set of parameters
ak run action anything-marco -q name:Paolo -q age:54
# you can add body and changing verb but will keep query parameters previously defined
ak run action anything-marco -b name:Paolo -b age:54 -v POST
```

### Testing an API call with multiple parameters

Sometimes, it can be useful to test an API with a set of query params or path params
```bash
ak run GET -u https://httpbin.org/anything -q 'name:Marco|Paolo' -q 'age:18|54'
```
It spawns the cartesian product of query params:
```
[00:00:00] 200 ✅  GET https://httpbin.org/anything?name=Marco&age=18
[00:00:01] 200 ✅  GET https://httpbin.org/anything?age=54&name=Marco
[00:00:00] 200 ✅  GET https://httpbin.org/anything?name=Paolo&age=18
[00:00:00] 200 ✅  GET https://httpbin.org/anything?name=Paolo&age=54
```
### Extract interesting values

You just made an API call. But you are especially interested in one key / subset of the payload.
An option can help you extract the data you are interested in:

```bash
# run the same query as before but extracting only the data attribute of the payload
ak run action anything-marco -b name:Paolo -b age:54 -v POST -e data
```

You can also save to the clipboard extracted data:
```bash
# run the same query as before but extracting only the data attribute of the payload
ak run action anything-marco -b name:Paolo -b age:54 -v POST -e data --clipboard
# below I hit Cmd + v after testing the command in my shell !
{\"age\":\"54\",\"name\":\"Paolo\"}
```

### Use variables that can be reused between API calls

You can assign variables to extracted data in order to be reused by next API calls

```bash
ak run action anything-marco -b name:Paolo -b age:54 -v POST -e data:BODY
ak run action anything-marco -b {{BODY}}
```

### Chain API calls to make it a flow

It can be useful to group successive API calls to make a unit of work, a flow that can be reused:

```bash
ak run action anything-marco -q 'name:Paolo;age:54' -c anything-marco -q 'name:Marco;age:18' --save anything-marco-paolo
```

> [!WARNING]
> Chaining flows has saveral drawbacks for now:
>   - you must specify the same parameters for all chained actions
>   - you can only chain named actions

### Add expectation to an API call

You can test extracted values to be equal to what you expect

```bash
 ak r action anything-marco -e args.name:NAME -e args.age:AGE --expect NAME:Marco --expect AGE:18

🐞 Analyzing results for anything-marco...
   🦄 ??Checking... Tests passed ✅
```

Some expectations are predefined:
- STATUS_CODE
- JSON_INCLUDE
  - ```bash
    # means we want extract all json response and save it as DATA variable
    ak run action get-todo -p id:1 -e '$:DATA' --expect 'DATA:JSON_INCLUDE({"id": 1})'
    ```
- JSON_EQ


Gives the following output

![tests results](img/tests.png "Test results")

### Save your tested API calls to a TestSuite
```bash
 ak r action anything-marco -e args.name:NAME -e args.age:AGE --expect NAME:Marco --expect AGE:18 --save-to-ts httpbin-ts
 ```

 then run the  `httpbin-ts` test suite:

```bash
 ak r ts httpbin-ts

 Running test suite httpbin-ts
🐞 Analyzing results for anything-marco...
   🦄 ??Checking... Tests passed ✅
  [00:00:00] [████████████████████████████████████████] 1/1 DONE
[00:00:00] 200 ✅  GET https://httpbin.org/anything?age=18&name=Marco
🎉 All tests passed!
 ```


## Project Management

You can group your API calls (or actions) into project instance. Project can hold configuration variables,
and thus can be reused in API calls definition.

### Create a new project

specifying for example the test url for your project
```bash
ak project new myproject --url https://jsonplaceholder.typicode.com -c api_key:MY_API_KEY
```
You can now add an action to your project

### Add an action to your project

You need to specify the name of your project, the name of your action,
the http verb, and the sub-url

```bash
ak project add-action myproject -n get-todo -v GET --url /todos/{id} -h 'x-api-key:{{api_key}}'
```

Basic support for loading **openapi spec file** (v3 only) to populate your project
and actions or **postman** collections
```bash
# if no servers are defined in your openapi spec, you can specify
# one using --test-url or --prod-url
ak project new myproject --from-openapi openapi.json
ak project new myproject --from-postman postman_collection.json
```

> [!WARNING]
> This is an experimental feature and may fail...

### List all projects
```bash
ak project list
```

### Get information about  your actions
```bash
ak project info myproject
```
Or using the ui to see all projects at once
```bash
ak project ui
```
![Project view](img/project_view.png "Project view")


### Run your action:
Then, action can be ran as usual:

```bash
ak run action get-todo -p id:1
```
```
Received response:
{
  "completed": false,
  "id": 1,
  "title": "delectus aut autem",
  "userId": 1
}
...
```

### Examples
Extract data from your response using jsonpath (not fully implemented yet)
```bash
ak run action get-todo -p id:1 -e completed
```
```
Request took: 286.501417ms
Status code: 200
Extraction of completed: false
```
You can also save the result in your clipboard
```bash
ak run action get-todo -p id:1 -e completed --clipboard
```
or ready to be used for grepping
```bash
ak run action get-todo -p id:1 -e completed --grep
```
You can use the grep option to filter out unwanted data
```bash
ak run action get-todo -p id:1 -e $ --grep >> result.json
```

## History

### List all requests history
```bash
ak history list
```
or using the ui
```bash
ak history ui
```
![History view](img/history.png "History view")


## Shell autocomplete
It is always more convenient to have autocomplete for your commands. Fortunately, clap
provides a way to generate a completion script for your shell.

```bash
ak completion bash > /usr/local/etc/bash_completion.d/apikrab
```
In order to get Project and action completion working, when using json/yaml Db backend, one have
to in top of his config completion file

```bash
	ACTIONS=()
    while read -r line; do
        filename=$(basename "$line" .json)
        ACTIONS+=("$filename")
    done < <(find ~/.config/qapi/projects -type f)

	PROJECTS=()
	while read -r line; do
        filename=$(basename "$line")
        PROJECTS+=("$filename")
    done < <(find ~/.config/qapi/projects/* -type d)
```

And report available values in all possible command places !

See contrib folder to see some examples.

Clap also provides completion for zsh, fish, powershell, elvish.
See the clap crate !


> [!INFO]
> For zsh, Generated script can autocomplete identifiers used for projects, actions, and test suites.


## Built with
- clap
- itertools
- sqlx
- reqwest
- serde
- serde_json
- ratatui
- colored
- tokio

## Ideas
- [ ] Share your project with others
- [ ] implements yaml/json collection instead of sqlite (human readable)
- [ ] Extend expectation mechanisms (regex, jsonpath, include, ...)
- [ ] Improve the ui

## Contributing
If you want to contribute to this project, you're welcome. Just open an issue or a pull request.

## License
MIT
