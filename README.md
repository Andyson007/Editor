# Editor
The scope of this project is to create a vim-inspired editor for collaboration work. It has support for live-sync between clients and password protection if needed.

The motivation from this project came from the way this feature is usually depicted in ads. Being roughly real time. I thought to myself that it wasn't really that way in products such as word. With this project I aimed to achieve similar levels of latency. 

## Getting started
You can run the a server instance using (square brackets for remote work):
```sh
cargo r -- server [--ip 0.0.0.0] ./<path>
```
You might have to open up a port for this to be acceble from other computers

And to run a client simply run:
```sh
cargo r -- client [--ip <ip-of-server>]
```
You can use `--help` to ask for more information.

If you are unfamiliar with vim bindings (I have minimal support at the moment) you can open up a help menu by typing `:help` when entering a client. (Don't worry about what actually gets displayed when you do this)
### Security
There is also an optional feature for security which you can access on the server side with
```sh
cargo r --features=security -- server
```
to connect with a client you will have to run
```sh
cargo r --features=security -- client
```
#### Adding a user
in order to add a user with a username on password run
```sh
 cargo r --features=security -- server --add-user
```

To have actual access to the documentation you probably want to run
```sh
cargo doc --no-deps --open
```
## Features
- [x] live syncronization
- [ ] support for across networks
And some general goals to keep in mind:
- Minimize latency between computers
- Add collaboration tools as I go on

## Reporting problems
- Check out [FAQ.md](FAQ.md) first
- Check if there is an issue for this already
- Check if updating fixes it
- Submit a new issue

## Contributing
For familiraity with the architecture check out [Architecture.md](Architecture.md).

Please fork the project to create pull requests. Pull requests are welcome!
