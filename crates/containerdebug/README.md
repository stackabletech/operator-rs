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
             

Here is an example of what it looks like on my Laptop:

```json
{
  "cpu_count": 8,
  "physical_core_count": 4,
  "total_memory": 50161664000,
  "free_memory": 2400735232,
  "available_memory": 35192512512,
  "used_memory": 14969151488,
  "total_swap": 53687087104,
  "free_swap": 53687087104,
  "used_swap": 0,
  "total_memory_cgroup": null,
  "free_memory_cgroup": null,
  "free_swap_cgroup": null,
  "system_name": "Arch Linux",
  "kernel_version": "6.7.8-arch1-1",
  "os_version": "Linux rolling Arch Linux",
  "host_name": "lars-laptop",
  "cpu_arch": "x86_64",
  "disks": [
    {
      "mount_point": "/",
      "total_space": 754416877568,
      "available_space": 55915364352
    },
    {
      "mount_point": "/home",
      "total_space": 754416877568,
      "available_space": 55915364352
    },
    {
      "mount_point": "/var/swap",
      "total_space": 754416877568,
      "available_space": 55915364352
    },
    {
      "mount_point": "/boot",
      "total_space": 2095079424,
      "available_space": 1896505344
    }
  ],
  "network_information": {
    "network_interfaces": {
      "wlan0": [
        "192.168.1.23",
        "fe80::90bf:60ff:fe78:836a"
      ],
      "lo": [
        "127.0.0.1",
        "::1"
      ],
      "virbr0": [
        "192.168.122.1"
      ]
    },
    "reverse_lookups": {
      "192.168.1.23": [
        "lars-laptop.localdomain."
      ],
      "192.168.122.1": [
        "lars-laptop.",
        "lars-laptop.local."
      ],
      "127.0.0.1": [
        "localhost."
      ],
      "::1": [
        "localhost."
      ],
      "fe80::90bf:60ff:fe78:836a": [
        "lars-laptop."
      ]
    },
    "forward_lookups": {
      "lars-laptop.": [
        "127.0.0.1"
      ],
      "lars-laptop.local.": [
        "192.168.1.23",
        "192.168.122.1"
      ],
      "localhost.": [
        "127.0.0.1"
      ],
      "lars-laptop.localdomain.": [
        "127.0.0.1"
      ]
    }
  }
}
```
