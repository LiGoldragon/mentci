# mentci

Mentci is the daemon for the programmable human approval surface. It keeps the
canonical UI state, lets clients subscribe to projected views, and routes
closed approval verdicts back toward the user's local criome.

This checkout currently contains the daemon-local Nexus and SEMA schemas. The
runtime daemon will be added after the external contract repositories have
canonical remotes, so the daemon can depend on generated nouns without local
path dependencies.
