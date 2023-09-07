# apicrab
CLI tools to manage your json api call in the terminal for fun only !

Written in rust 🦀

Tested on MacOs only

## Philosophy
The goal of this project is to provide a simple tool to manage your json api call in the
terminal. It's not meant to be used in production. It's just a fun project to learn rust.

First notion is the **project**. A project has a name and root urls for an api to test, and
optionally a set of **configuration variables**. You can then attach **actions** to your
project.

An action represents a specific endpoint of your api. It has a name, a method (http verb), 
an url.

You can run an action with a set of **parameters**. A parameter is a key value pair. The key
is the name of the parameter, the value is the value of the parameter. The value can be a
string or a json object, just as Curl.

A **flow** represents a set of actions to run with predefined parameters. You can chain 
actions to create a flow.

## Features
- [x] Create a new project
- [x] Add an action to your project
- [x] Run an action
- [x] Extract data from your response using jsonpath
- [x] Chain actions
- [ ] Test your action

## Build from source
Install rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Clone the repo
```bash
git clone
```
Build the project
```bash
cargo build --release
```
Add the binary to your path
```bash
export PATH=$PATH:/path/to/apicrab/target/release
```

## Usage
```bash
apicrab --help
```

## Example

Create a new project
```bash
apicrab project new myproject --test-url https://jsonplaceholder.typicode.com

```

Add an action to your project
```bash
apicrab project add-action myproject get-todo -v GET --url /todos/{id}
```

Run your action
```bash
apicrab run action get-todo -p id:1
```
```
Request took: 265.607263ms
Status code: 200
Action updated
Received response: 
{
  "completed": false,
  "id": 1,
  "title": "delectus aut autem",
  "userId": 1
}
...
```

Extract data from your response using jsonpath (not fully implemented yet)
```bash
apicrab run action get-todo -p id:1 -e completed
```
```
Request took: 286.501417ms
Status code: 200
Action updated
Extraction of completed: false 
```

Save your action as flow to avoid repeating yourself. This one is fairly simple.
```bash
apicrab run action get-todo -p id:1 -e completed:COMPLETED --save-flow get-todo
```

Then you just have to run
```bash
apicrab run flow get-todo
```

Flow are especially useful to test your api. You can add expectations to your flow.
```bash
apicrab test-suite new mytest
apicrab test-suite add-flow mytest get-todo --expect COMPLETED:false --expect STATUS_CODE:200

apicrab run test-suite mytest
```



