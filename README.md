# 😁🦀 apicrab
CLI tools to manage your json api call in the terminal for fun only !


![apicrab](img/apicrab.png "apicrab")

## Philosophy


# TL;DR
> **You can create a project, add actions to it, run them, chain them, save them as flows, and
> test them.**


The goal of this project is to provide a simple tool to manage your json api call in the
terminal. It is still in very early stage of development and is not intended to be used in
production.


The first concept is the **project**. A project has a name and root urls for an api to test, and
optionally a set of **configuration variables**. You can then attach **actions** to your
project.

An **action** represents a specific endpoint of your api. It has a name, a method (http verb), 
an url, and optionally a body (in case of static body). You can also specify a set of **headers**.

You can run an action with a set of **parameters** such body, path parameters, and query 
parameters.

A **flow** represents an action or chained actions to run with predefined parameters.

Finally, a **test suite** is a set of flows with expectations. You can run a test suite to
check if your api is still working as expected.

Some commands have an ui mode (history, project info). See the help for more information.

## Features
- [x] Create a new project
- [x] Add an action to your project
- [x] Run an action
- [x] Extract data from your response using jsonpath (not fully implemented yet)
- [x] Chain actions
- [x] Test your action

> [!WARNING]
> Tested on MacOs only

## Installation
### Build from source
Install rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Clone the repo
```bash
git clone git@github.com:jerkos/apicrab.git
```
Build the project in release mode
```bash
cargo build --release
```
Add the binary to your path
```bash
export PATH=$PATH:/path/to/apicrab/target/release
```

### Download the binary

Grab the latest release from the [release page](https://github.com/jerkos/apicrab/releases)
for your platform (linux or darwin for the moment), and put in your path.

## Usage
```bash
apicrab --help
```
Works also for all subcommands, e.g.
```bash
apicrab project --help
```
![Help view](img/help.png "Help view")


## Create a new project

specifying for example the test url for your project
```bash
apicrab project new myproject --test-url https://jsonplaceholder.typicode.com

```
You can now add an action to your project

## Add an action to your project

You need to specify the name of your project, the name of your action, 
the http verb, and the sub-url
```bash
apicrab project add-action myproject -n get-todo -v GET --url /todos/{id}
```
Basic support for loading openapi spec file (v3 only) to populate your project
and actions
```bash
# if no servers are defined in your openapi spec, you can specify 
# one using --test-url or --prod-url
apicrab project new myproject --from-openapi openapi.json
```

## Getting information about projects

### List all projects
```bash
apicrab project list
```

### Get information about  your actions
```bash
apicrab project info myproject
```
Or using the ui to see all projects at once
```bash
apicrab project ui
```
![Project view](img/project_view.png "Project view")


## Run your action:
- with path parameters, syntax is `-p name:value`
- with query parameters, syntax is `-q name:value`
- with body, syntax is `-b name:value` or `-b '{"name": "value"}'`
```bash
apicrab run action get-todo -p id:1
```
```
Request took: 265.607263ms
Status code: 200
Received response: 
{
  "completed": false,
  "id": 1,
  "title": "delectus aut autem",
  "userId": 1
}
...
```

### Chain actions
```
# project as been created with configuration parameters CLIENT_ID and CLIENT_SECRET
apicrab project add-action myproject -n authent\n
--static-body '{client_id:"{CLIENT_ID}", "client_secret": "{CLIENT_SECRET", "grant-type": "client_credentials"}' \n
-u oauth/token --form

apicrab project add-action myproject -n search_by_name\n
-u todos?name={name}
-h 'Authorization: Bearer {ACCESS_TOKEN}'

apicrab run action authent -q '' -e access_token:ACCESS_TOKEN\n
--chain search_by_name -q 'name:Buy tomatoes' -e $ --save-as get-todo-by-name-flow

apicrab run flow get-todo-by-name-flow
```

### Run action concurrently specifying several path params / query params:
```bash
apicrab run action get-todo -p id:1 -p id:2 -p id:3
# or shortier
apicrab run action get-todo -p 'id:1|2|3'
# or with query params
apicrab run action get-todo -p 'id:1|2|3' -q 'completed:true|false'
# 🔥 launch the cartesian product of all params !
```

## Results handling
Extract data from your response using jsonpath (not fully implemented yet)
```bash
apicrab run action get-todo -p id:1 -e completed
```
```
Request took: 286.501417ms
Status code: 200
Extraction of completed: false 
```
You can also save the result in your clipboard
```bash
apicrab run action get-todo -p id:1 -e completed --clipboard
```
or ready to be used for grepping
```bash
apicrab run action get-todo -p id:1 -e completed --grep
```
You can use the grep option to filter out unwanted data
```bash
apicrab run action get-todo -p id:1 -e $ --grep >> result.json
```

## List all requests history
```bash
apicrab history list
```
or using the ui
```bash
apicrab history ui
```
![History view](img/history.png "History view")

## Save your action as flow to avoid repeating yourself. 

This one is fairly simple.
```bash
apicrab run action get-todo -p id:1 -e completed:COMPLETED --save-flow get-todo
```

Then you just have to run
```bash
apicrab run flow get-todo
```

Flow are especially useful to test your api. You can add expectations to your flow.

## Test your api

```bash
apicrab test-suite new mytest
apicrab test-suite add-flow mytest -n get-todo --expect COMPLETED:false --expect STATUS_CODE:200

apicrab run test-suite mytest
```
Some expectations are predefined:
- STATUS_CODE
- JSON_INCLUDE
  - ```bash
    # means we want extract all json response and save it as DATA variable
    apicrab run action get-todo -p id:1 -e '$:DATA' --save-flow get-todo-2
    
    # then we can use it in our test suite, expecting DATA to include {"id": 1}
    apicrab test-suite add-flow mytest -n get-todo-2 --expect 'DATA:JSON_INCLUDE({"id": 1})'
    ```
- JSON_EQ


Gives the following output

![tests results](img/tests.png "Test results")


## Shell autocomplete
It is always more convenient to have autocomplete for your commands. Fortunately, clap
provides a way to generate a completion script for your shell.
    
```bash
apicrab completion bash > /usr/local/etc/bash_completion.d/apicrab
```
Clap also provides completion for zsh, fish, powershell, elvish. 
See the clap crate !


> [!INFO]
> For zsh, Generated script can autocomplete identifiers used for projects, actions, flows, and test suites.


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

- [ ] Add a way to load your project from a file (postman collection ?)
- [ ] Share your project with others
- [ ] Extend expectation mechanisms (regex, jsonpath, include, ...)
- [ ] Improve the ui


## Contributing

If you want to contribute to this project, you're welcome. Just open an issue or a pull request.

## License
MIT