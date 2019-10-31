# Keybase Chat Terminal UI

This is a terminal UI app for interacting with keybase chat. It is not very
usable atm.

To run with debug logging. Logs to stderr because stdout is for the UI.
```
RUST_BACKTRACE=1 RUST_LOG=keybase_chat_tui cargo run 2>out.log

# In another shell
tail -f out.log
```

## Future Enhancements

* Show which conversation is currently displayed
* Show which conversation has unread messages
* Cache the chat messages or use a ring buffer or something
* Support attachments and other message types
* Think harder about how focus should work

## Bugs

* Empty conversations don't clear the chat area
