# General Platform Architecture


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
