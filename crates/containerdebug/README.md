# Container Support Helper

This is a tool meant to make the life of a support engineer easier when working with docker containers.

We often face issues where we would have loved to know more details about the environment a container is running in.
This tool is meant to print as much information as possible to stdout (later possibly also to a log file) to aid in debugging:

It currently prints:
- CPU core count
- Memory (free, total, available)
- CGroup Limits
- Swap information
- Host name
- Kernel information
- CPU architecture
- Mount points and available space on those
- List of all network interfaces with names and IP addresses
- Reverse Lookup for those IP addresses (IP Address => List of hostnames)
- Forward lookup for those hostnames (Hostname => List of IP addresses)
