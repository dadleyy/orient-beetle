#include "redis-manager.hpp"

namespace redismanager {

  Manager::Manager(const char * host, const uint32_t port, const char * auth):
    _redis_host(host),
    _redis_port(port),
    _redis_auth(auth),
    _paused(false),
    _state(Disconnected { tick: 0 })
  {
  }

  uint16_t Manager::copy(char * destination, uint16_t max) {
    if (_state.index() != 1) {
      return 0;
    }

    Connected * c = std::get_if<1>(& _state);

    return c->copy(destination, max);
  }

  std::optional<Manager::EManagerMessage> Manager::frame(
      std::optional<wifimanager::Manager::EManagerMessage> &message
  ) {
    switch (_state.index()) {
      case 0: {
        Disconnected * d = std::get_if<0>(&_state);

        if (d->update(message) || _paused) {
#ifndef RELEASE
          log_d("attempting to move from disconnect to connected");
#endif

          _paused = false;
          _state.emplace<Connected>();
        }
        break;
      }
      case 1:
        Connected * c = std::get_if<1>(&_state);

        // If we've been completely disconnected, restart.
        if (message == wifimanager::Manager::EManagerMessage::Disconnected) {
          _state.emplace<Disconnected>();
          _paused = false;
          return Manager::EManagerMessage::ConnectionLost;
        }

        // If we're paused and we've got anything but a resume, do nothing.
        if (_paused && message != wifimanager::Manager::EManagerMessage::ConnectionResumed) {
          return std::nullopt;
        }

        // If we've received an interruption, pause immediately.
        if (message == wifimanager::Manager::EManagerMessage::ConnectionInterruption) {
          _paused = true;
#ifndef RELEASE
          log_d("wifi connection was interrupted, pausing all requests");
#endif

          return std::nullopt;
        }

        // If we're paused and are about to resume, reset us to disconnected. The next frame will bounce us
        // back into connected.
        if (_paused && message == wifimanager::Manager::ConnectionResumed) {
          _state.emplace<Disconnected>();
          return Manager::EManagerMessage::ConnectionLost;
        }

        return c->update(_redis_host, _redis_auth, _redis_port);
    }

    return std::nullopt;
  }

  uint16_t Manager::Connected::copy(char * destination, uint16_t size) {
    if (_cursor == 0 || _certified != ECertificationStage::Identified) {
      return 0;
    }

    uint16_t amount = _cursor < size ? _cursor : size;
    memcpy(destination, _framebuffer, amount);
    _cursor = 0;
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
    return amount;
  }

  std::optional<Manager::EManagerMessage> Manager::Connected::update(
      const char * _redis_host,
      const char * _redis_auth,
      uint32_t _redis_port
  ) {
    _cursor = 0;

    // If we have not attempted to connect + certify, we need to apply our root certificates
    // and open the tcp connection using our `client.`
    if (_certified == ECertificationStage::NotRequested) {
      _certified = ECertificationStage::CerificationRequested;

#ifndef RELEASE
      log_d("attempting to certify redis connection");
#endif
      extern const uint8_t redis_root_ca[] asm("_binary_embeds_redis_host_root_ca_pem_start");

      _client.setCACert((char *) redis_root_ca);

#ifndef RELEASE
      log_d("attempting to establish connection with redis %s:%d", _redis_host, _redis_port);
#endif

      int result = _client.connect(_redis_host, _redis_port);

      if (result != 1) {
#ifndef RELEASE
        log_e("unable to establish connection - %d", result);
#endif
        return Manager::EManagerMessage::FailedConnection;
      }

#ifndef RELEASE
      log_d("successfully connected to redis host/port ('%s')", _redis_auth);
#endif

      // At this point we have a valid tcp connection using tls and can start writing our redis
      // commands. Start by sending the `AUTH` command.
      char auth_command [256];
      memset(auth_command, '\0', 256);
      sprintf(auth_command, "*2\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n", strlen(_redis_auth), _redis_auth);
      size_t written = _client.print(auth_command);

#ifndef RELEASE
      log_d("wrote %d bytes on first message", written);
#endif

      // Return early, delay read for another frame at least.
      return std::nullopt;
    }

    // If we're officially "certified" with the server, it is time to request our identity.
    if (_certified == ECertificationStage::Certified) {
      _certified = ECertificationStage::IdentificationRequested;
#ifndef RELEASE
      // At this point we've got a connection and have successfully authorized ourselves.
      log_d("writing pop command for next frame");
#endif
      _client.print(REDIS_REGISTRATION_POP);

      return std::nullopt;
    }

    // Clear out the framebuffer.
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);

    // Read everything we can off our client.
    while (_client.available() && _cursor < FRAMEBUFFER_SIZE - 1) {
      _framebuffer[_cursor] = (char) _client.read();
      _cursor += 1;
    }

    // If we are not certified, assume this is our first `+OK` response pulled in off the 
    // `AUTH` command that was issued previously.
    if (_certified == ECertificationStage::CerificationRequested) {
#ifndef RELEASE
      log_d("not yet certified with server, skipping pop");
#endif

      // If we had no data, do nothing (we're still waiting for a response).
      if (_cursor == 0) {
        return std::nullopt;
      }

      if (strcmp(_framebuffer, "+OK\r\n") == 0) {
#ifndef RELEASE
        log_d("successfully authorized connection to redis");
#endif
        _certified = ECertificationStage::Certified;

        return Manager::EManagerMessage::EstablishedConnection;
      }

      // We've received some unknown message while not certified, ignore.
      return std::nullopt;
    }

    // If we popped something off the queue and it was empty, do nothing.
    if (strcmp(_framebuffer, "$-1\r\n") == 0 || _cursor == 0) {
#ifndef RELEASE
      if (_certified == ECertificationStage::IdentificationRequested) {
        log_e("unable to pull device id, is registrar running?");
      }
#endif

      // If we had nothing but we're identified, write our next pop before moving on.
      if (_certified == ECertificationStage::Identified) {
        write_pop();
      }

      _cursor = 0;
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      return std::nullopt;
    }

    // At this point, we have what appears to be a valid message sitting in our framebuffer.
    // Now we will attempt to parse out the actual message contents (later copied back into
    // the framebuffer).
    uint8_t stage = 0, len = 0, size = 0;
    char * n = _framebuffer;
    char message [FRAMEBUFFER_SIZE];
    memset(message, '\0', FRAMEBUFFER_SIZE);
    bool isint = false;

    while (*n != '\0' && (stage == 0 ? len < 2 : (stage == 1 ? len < 20 : len < size))) {
      char tok = *n;
      n++;

      if ((tok == '$' || tok == ':') && stage == 0) {
        stage = 1;

        // Special casing the not-so-valuable integer response. At some point it will be
        // nice to handle this in some way but for the time being it is only valuable for
        // the final registration `rpush` that is issued.
        if (tok == ':') {
          isint = true;
        }

        continue;
      }

      // After we've identified ourselves as a bulk string, the first time we see a newline
      // means that we've parsed the size of the message.
      if (stage == 1 && tok == '\r') {
        // Skip the '\n' that will follow
        n++;

        // Parse the contents of our current message as the size.
        for (uint8_t i = 0; i < len; i++) {
          size = (size * 10) + (message[i] - '0');
        }

        // We're now parsing the "actual" content.
        stage = 2;
        len = 0;

#ifndef RELEASE
        log_d("parsed message length - %d (from %s)", size, message);
#endif

        // Clear out our buffer in preparation for the real content.
        memset(message, '\0', FRAMEBUFFER_SIZE);
        continue;
      }

      message[len] = tok;
      len += 1;
    }

    if (len != size || len == 0 || stage != 2) {
#ifndef RELEASE
      if (isint == false) {
        log_e("unable to parse message - '%s'", message);
      }
#endif
      _cursor = 0;
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      return std::nullopt;
    }

#ifndef RELEASE
    log_d("parsed message '%s' (len %d)", message, len);
#endif

    // TODO: here we are copying our parsed message back onto the memory we've allocated for
    // our framebuffer. It is definitely possible that this is not necessary.
    memcpy(_framebuffer, message, len);
    if (len < FRAMEBUFFER_SIZE) {
      _framebuffer[len] = '\0';
    }

    if (_certified == ECertificationStage::IdentificationRequested) {
#ifndef RELEASE
      log_d("assuming '%s' is our identity", _framebuffer);
#endif
      memcpy(_device_id, message, len < MAX_ID_SIZE ? len : MAX_ID_SIZE);
      _certified = ECertificationStage::Identified;

      char buffer [256];
      memset(buffer, '\0', 256);
      sprintf(buffer, "*3\r\n$5\r\nRPUSH\r\n$4\r\nob:i\r\n$%d\r\n%s\r\n", strlen(_device_id), _device_id);
      uint16_t written = _client.print(buffer);

#ifndef RELEASE
      log_d("wrote %d identification reservation bytes", written);
#endif

      return std::nullopt;
    }

    if (_certified == ECertificationStage::Identified) {
#ifndef RELEASE
      log_d("writing our identified pop command (%s)", _device_id);
#endif

      uint16_t written = write_pop();

#ifndef RELEASE
      log_d("wrote '%d' bytes as identified user", written);
#endif
    }

    return std::optional(Manager::EManagerMessage::ReceivedMessage);
  }

  Manager::Connected::Connected():
    _certified(ECertificationStage::NotRequested),
    _cursor(0),
    _write_delay(0) {
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      memset(_device_id, '\0', MAX_ID_SIZE + 1);
  }

  Manager::Connected::~Connected() {
#ifndef RELEASE
    log_d("cleaning up redis client connection");
#endif

    _client.stop();
  }

  inline uint16_t Manager::Connected::write_pop(void) {
    // Only actually write a pop every 10 times we request one...
    if (_write_delay++ < 10) {
      return 0;
    }
    _write_delay = 0;

    char buffer [256];
    memset(buffer, '\0', 256);
    uint8_t keysize = strlen(_device_id) + 3;
    sprintf(buffer, "*2\r\n$4\r\nLPOP\r\n$%d\r\nob:%s\r\n", keysize, _device_id);
    return _client.print(buffer);
  }

  bool Manager::Disconnected::update(std::optional<wifimanager::Manager::EManagerMessage> &message) {
    if (message == wifimanager::Manager::EManagerMessage::Connected) {
      return true;
    }

    tick += 1;
    if (tick > 100) {
      tick = 0;
    }

    return false;
  }
}
