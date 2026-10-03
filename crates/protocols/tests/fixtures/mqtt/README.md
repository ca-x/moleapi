# Mature MQTT broker fixtures

The MQTT integration tests spawn isolated Mosquitto 2.x processes (MQTT 3.1.1 and 5), without Docker or changing a system service. Each process uses a temporary configuration and an ephemeral loopback port above 18891. Set `MOLEAPI_MOSQUITTO_BIN` to an installed or extracted broker binary. TLS and Basic authentication tests additionally use `MOLEAPI_MOSQUITTO_PASSWD_BIN` (or its sibling `mosquitto_passwd`).

```
MOLEAPI_MOSQUITTO_BIN=/path/to/mosquitto cargo test -p moleapi-protocols --test mqtt -- --ignored --test-threads=1
```

For this development session Mosquitto 2.1.2-2 and cjson 1.7.19-1 Arch packages were downloaded and extracted under `/tmp/moleapi-mqtt-broker`; nothing was installed. Use `LD_LIBRARY_PATH=/tmp/moleapi-mqtt-broker/usr/lib`, broker `/tmp/moleapi-mqtt-broker/usr/bin/mosquitto`, and the sibling password tool. Packages retain their upstream EPL/EDL licenses in the extracted tree. These binaries are external test tooling, not distributed application assets.

TLS creates a temporary self-signed certificate: verified mode must reject it and explicit verification opt-out must connect. Original-hostname pinning is also covered by connecting localhost using the address extension without rewriting the host to an IP. This demonstrates opt-out and rejection; custom client certificates/shared TLS settings remain a separate capability.
