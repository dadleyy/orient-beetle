# General Platform Architecture

At a high level, the architecture of the platform is based on four components - a web backend,
a web frontend, and the individual devices, with a redis database being leveraged as a message
broker and authentication system for the devices:

![general-architecture](https://user-images.githubusercontent.com/1545348/193424612-715ae3a7-9215-4199-8a95-07fc158ad794.png)

### Device Authorization

In order for devices to receive messages, they must first authenticate with the redis service,
which is what we're using to act as a message broker. This authentication scheme is handled by
using redis ACL entries, with a global provisioning ACL burned into devices during firmware
flashing.

```mermaid
sequenceDiagram
    Embedded Device->>Redis: AUTH <burn-in-acl>
    Redis-->>Embedded Device: +OK
    Embedded Device->>Redis: LPOP available_ids
    Redis-->>Embedded Device: 4af2bbd1
    Embedded Device->>Redis: AUTH 4af2bbd1
    Redis-->>Embedded Device: +OK
    note over Embedded Device,Redis: authorized for LPOP
    loop Every Second
        Embedded Device->>Redis: LPOP messages:4af2bbd1
        Redis-->>Embedded Device: <state-message>
    end
```

[← README](../../README.md)
