### Redis Help

Connecting to a digital ocean redis instance requires tls by default. This means that the
certificates must be available to the firmware at runtime in order to leverage the arduino
`WiFiClientSecure` class.

To fetch the certificates from digital ocean, run:

```
openssl s_client -showcerts -connect <redis-hostname>:<redis-port> </dev/null
# example:
openssl s_client -showcerts -connect db-redis-nyc1-78553-do-user-6191575-0.b.db.ondigitalocean.com:25061 </dev/null
```

[reference](https://www.baeldung.com/linux/ssl-certificates)
