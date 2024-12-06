# Editor
The scope of this project is to create a vim-inspired editor for collaboration work. The motivation from this project came from the way this feature is usually depicted in ads. Being roughely real time. I aim to replicate someting like this.

# How to use
You can run the a server for this client using 
```sh
cargo r -- server
```
and a client with
```sh
cargo r -- client
```
You can use --help to ask for more information.
If you are unfamiliar with vim bindings (I have minimal support at the moment) you can open up a help menu by typing `:help` when entering a client. (Don't worry about what actually gets displayed when you do this)
## Security
There is also an optional feature for security which you can access on the server side with
```sh
cargo r --features=security -- server
```
to connect with a client you will have to run
```sh
cargo r -- client --password
```


## Goals
- Minimize latency between computers
- Add collaboration tools as I go on
