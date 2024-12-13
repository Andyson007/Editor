This Editor has multiple sub crates.
- [append_only_str](#Append_only_str). A string which can only be appended to
- [btep](#Btep). Responsible for serialization traits for communacition between server and client
- [piece-table](#Piece-table). Uses append_only_str to create a custom piece-table implementation
- [text](#Text). A wrapper around text making handling clients easier
- [server](#Server). Handles the server side.
- [client](#Client). Handles the client side including rendering and such.
- [utils](#Utils). Utilities
- [bundled](#Bundled). Bundles together the server and client to create a single binary.


# Append_only_str
A string type which can only be appended to.
This is useful for storing append buffers.

# Btep
Rhe \[B\]inary \[T\]ext \[E\]ditor \[P\]rotocol. 
The notable feature here are the `Serialize` and `Deserialize` traits defined to easily be able to convert stuff to u8 streams and sent between the server and client

# Piece-table
The piece table implementation for this project.
This implementation of a piece table differs from the regular implementation as it uses multiple append buffers for each of the clients.
This allows inserts to be handled without considering the whole buffer

# Text
A (relatively small) wrapper around the [piece table](#Piece-table) which defines a `Client`. A `Client` can edit its portion of the piece table
## Table
Another major part of text is its `Table`. The table is a wrapper aronud a Linked list, but with loser ownership requirenments.
It allows for multiple elements to be read/edited at the same time and can lock down the entire table for reading and reordering elements.

# Server
Creates a TcpListener and handles all incoming requests.
It handles writing to the file, and propagating requests between clients.

## Security
Security is an optional feature of server. It creates a database for user and password storage. (This is mostly because my school required there to be a database in the project)
# Client
The client side of the project.
The client has to render text to the user and handle inputs from the client.
# Utils
A place when you have to create a utility for something, but are able to make it generic (enough).
# Bundled
Bundles together the [client](#Client) and the [server](#Server).
This module is also responsible for handling CLI-arguments.
