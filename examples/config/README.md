# vSMTP Configuration format

This folder contain example of vSMTP configuration.

At startup of the server, the configuration is read and entirely parsed,
producing an error if the format is invalid or a field value is incorrect.

All the possible errors are detected at startup making the server never
failing if the configuration is successfully loaded.

All the field are optional, and defaulted if missing. See the [minimal] config.

* [simple](./simple.toml)
* [tls](./tls.toml)
* [logging](./logging.toml)
* [secured](./secured.toml)
* [antivirus](./antivirus.toml)

[minimal]: ./minimal.toml
