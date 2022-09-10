#include "redis-manager.hpp"

namespace redismanager {

  Manager::Manager(std::tuple<const char *, const uint32_t, std::pair<const char *, const char *>> config):
    _redis_host(std::get<0>(config)),
    _redis_port(std::get<1>(config)),
    _redis_auth(std::get<2>(config)),
    _paused(false),
    _state(Disconnected {})
  {
  }

  void Manager::begin(void) {
    _preferences.begin("beetle-redis", false);
  }

  // Given a destination, fill it with the contents of our connected state if available.
  uint16_t Manager::copy(char * destination, uint16_t max) {
    if (_state.index() != 1) {
      return 0;
    }

    Connected * c = std::get_if<1>(& _state);

    return c->copy(destination, max);
  }

  // The main "tick" function of our redis manager.
  std::optional<Manager::EManagerMessage> Manager::frame(
    std::optional<wifimanager::Manager::EManagerMessage> &message,
    uint32_t current_time
  ) {
    switch (_state.index()) {
      // When disconnected - check to see if the wifi manager has connected.
      case 0: {
        Disconnected * disconnected_state = std::get_if<0>(&_state);

        if (disconnected_state->update(message) || _paused) {
          log_d("attempting to move from disconnect to connected");

          _paused = false;
          _state.emplace<Connected>(&_preferences);
        }
        break;
      }

      // If we're connected, continue this frame by attempting an update from the connected state.
      // We'll also be handling wifi disconnects in this state.
      case 1: {
        Connected * connected_state = std::get_if<1>(&_state);

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
          log_d("wifi connection was interrupted, pausing all requests");

          return std::nullopt;
        }

        // If we're paused and are about to resume, reset us to disconnected. The next frame will
        // bounce us back into connected.
        if (_paused && message == wifimanager::Manager::ConnectionResumed) {
          _state.emplace<Disconnected>();
          return Manager::EManagerMessage::ConnectionLost;
        }

        // At this point, the message received from the wifi manager is not something we're really
        // concerned with, so apply our update.
        return connected_state->update(_redis_host, _redis_auth, _redis_port, current_time);
      }
    }

    return std::nullopt;
  }

  // Connected state copy - consume the framebuffer into our destination.
  uint16_t Manager::Connected::copy(char * destination, uint16_t size) {
    if (_cursor == 0 || _authorization_stage != EAuthorizationStage::FullyAuthorized) {
      return 0;
    }

    uint16_t amount = _cursor < size ? _cursor : size;
    memcpy(destination, _framebuffer, amount);
    _cursor = 0;
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
    return amount;
  }

  // The main update method here is responsible for potentially receiving and writing
  // data over tcp to our redis server.
  std::optional<Manager::EManagerMessage> Manager::Connected::update(
    const char * _redis_host,
    const std::pair<const char *, const char *> & _redis_auth,
    uint32_t _redis_port,
    uint32_t current_time
  ) {
    _cursor = 0;

    if (_strange_thing_count > 10) {
      log_e("too many strange things have happened. resetting the tcp connection");
      reset();
      return std::nullopt;
    }

    if (_timer.update(current_time) != 1) {
      return std::nullopt;
    }

    switch (_authorization_stage) {
      // If we have not attempted to connect + certify, we need to apply our root certificates
      // and open the tcp connection using our `client.`
      case EAuthorizationStage::NotRequested: {
        return connect(_redis_host, _redis_auth, _redis_port);
      }

      // If we have received an `+OK` from our burn-in credential authorization attempt, time to
      // request an id by writing our pop.
      case EAuthorizationStage::AuthorizationReceived: { 
        log_d("requesting new id from registrar using burn-in credentials");
        _authorization_stage = EAuthorizationStage::IdentificationRequested;
        _client.print(REDIS_REGISTRATION_POP);
        return std::nullopt;
      }

      // States that are expecting to read _something_ off the tcp connection.
      case IdentificationRequested:
      case AuthorizationAttempted:
      case AuthorizationRequested:
      case FullyAuthorized:
        break;
    }

    // Clear out the framebuffer.
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);

    // Read everything we can off our client.
    while (_client.available() && _cursor < FRAMEBUFFER_SIZE - 1) {
      // TODO: `_client.read((uint8_t *) _framebuffer, FRAMEBUFFER)` might be a better choice here
      _framebuffer[_cursor] = (char) _client.read();
      _cursor += 1;
    }

    // Explicitly handle `wrongpass`.
    if (strcmp(_framebuffer, WRONG_PASS_ERR) == 0) {
      log_e("wrongpass received, resetting client");
      reset();

      return std::nullopt;
    }

    // Explicitly handle bad permission errors.
    if (strcmp(_framebuffer, NO_PERM_ERR) == 0) {
      log_e("permissions lost, resetting client");
      reset();

      return std::nullopt;
    }

    bool is_empty = strcmp(_framebuffer, EMPTY_RESPONSE) == 0;
    bool is_ok = strcmp(_framebuffer, OK) == 0;

    switch (_authorization_stage) {
      // Neither of these states should be dealing with parsing incoming messages from
      // our tcp stream.
      case EAuthorizationStage::NotRequested:
        return std::nullopt;

      case EAuthorizationStage::AuthorizationReceived:
        return std::nullopt;

      case EAuthorizationStage::AuthorizationAttempted: {
        // The only thing we're expecting here is an `+OK`. Anyhing else means something strange
        // has happened.
        if (is_ok) {
          log_d("received 'OK' during certification stage '%d'", _authorization_stage);
          _authorization_stage = EAuthorizationStage::FullyAuthorized;
          _strange_thing_count = 0;
          return std::nullopt;
        }

        if (_cursor > 0) {
          log_e("received strange response after attempting device-specific acl: '%s'", _framebuffer);
          _strange_thing_count = _strange_thing_count + 1;
        }

        return std::nullopt;
      } 

      case EAuthorizationStage::AuthorizationRequested: {
        // The only thing we're expecting here is an `+OK`. Anyhing else means something strange
        // has happened.
        if (is_ok) {
          log_d("successfully authorized connection to redis, will attempt to pull id on next update");
          _authorization_stage = EAuthorizationStage::AuthorizationReceived;
          _strange_thing_count = 0;
          return Manager::EManagerMessage::EstablishedConnection;
        }

        if (_cursor > 0) {
          log_e("received strange response after attempting burn-in acl: '%s'", _framebuffer);
          _strange_thing_count = _strange_thing_count + 1;
        }

        return std::nullopt;
      }
      
      case EAuthorizationStage::FullyAuthorized: {
        if (_empty_identified_reads > MAX_EMPTY_READ_RESET) {
          log_e("too many empty reads while in authorized exchange, resetting");
          reset();
          return std::nullopt;
        }

        // If nothing was pulled this frame, we either want to increment our count of
        // "hey I was expecting a message", or we want to write a new message.
        if (_cursor == 0) {
          if (_pending_response) {
            _empty_identified_reads += 1;
          } else {
            write_message(current_time);
          }
          return std::nullopt;
        }

        // Its all good if we received an empty string. Just go ahead and reset our
        // count, writing a new fresh message.
        if (is_empty) {
          _empty_identified_reads = 0;
          _pending_response = false;
          log_d("nothing-burger, going ahead with potential message send");
          write_message(current_time);
          return std::nullopt;
        }

        auto parse_result = parse_framebuffer();

        // Be very specific about what we consider a valid message.
        if (parse_result != 2) {
          _pending_response = false;
          log_e("strange parse result while authorized - '%s'", _framebuffer);
          _strange_thing_count = _strange_thing_count + 1;
          return std::nullopt;
        }

        strcpy(_framebuffer, _parsed_message);

        if (strcmp(_framebuffer, "__reset__") == 0) {
          _cached_reset_count = MAX_RESETS_RECREDENTIALIZE + 1;
          reset();
          return std::nullopt;
        }

        // At this point we have a real message; reset this empty read counter, send
        // another message and move along.
        _empty_identified_reads = 0;
        _pending_response = false;
        write_message(current_time);
        return EManagerMessage::ReceivedMessage;
      }

      case EAuthorizationStage::IdentificationRequested: {
        if (is_empty) {
          log_e("empty response from identification request, is registrar running?");
          _strange_thing_count = _strange_thing_count + 1;
          return std::nullopt;
        }

        auto parse_result = _cursor > 0 ? parse_framebuffer() : 0;

        if (parse_result != 2) {
          _strange_thing_count = _strange_thing_count + 1;
          return std::nullopt;
        }

        log_d("assuming '%s' is our identity", _parsed_message);

        _device_id_len = strlen(_parsed_message);
        memcpy(_device_id, _parsed_message, _device_id_len);

        size_t stored_id_len = _preferences->putString("device-id", _device_id);
        log_d("stored device id (%d bytes)", stored_id_len);

        // We now have our individual identity.
        _authorization_stage = EAuthorizationStage::FullyAuthorized;

        log_d("writing auth command with new id %s", _device_id);

        // Now that we have a valid device id, authorize as that.
        memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
        sprintf(
          _outbound_buffer,
          "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
          strlen(_device_id),
          _device_id,
          strlen(_device_id),
          _device_id
        );
        _client.print(_outbound_buffer);
        memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);

        _authorization_stage = EAuthorizationStage::AuthorizationAttempted;

        return Manager::EManagerMessage::IdentificationReceived;
      }
    }

    return std::nullopt;
  }

  inline uint8_t Manager::Connected::parse_framebuffer(void) {
    memset(_parsed_message, '\0', PARSED_MESSAGE_SIZE);

    uint8_t parsing_stage = 0;
    uint8_t stage_length = 0;
    uint8_t parsed_length = 0;
    char * current_token = _framebuffer;

    while (*current_token != '\0') {
      // In each stage, we want to check if we're still "safe" with regards to our expectations
      // about the size of thing we're reading, and how much we've already read.
      bool is_safe = parsing_stage == 0 
        ? stage_length < 2 
        : (parsing_stage == 1 ? stage_length < 20 : stage_length < parsed_length);

      // Success!
      if (parsing_stage == 2 && stage_length == parsed_length) {
        break;
      }

      if (!is_safe) {
        log_e("aborting parse of strange message - '%s'", _framebuffer);
        break;
      }

      char tok = *current_token;
      current_token++;

      if ((tok == '$' || tok == ':') && parsing_stage == 0) {
        parsing_stage = 1;
        stage_length = 0;
        continue;
      }

      if (parsing_stage == 1 && tok == '\r') {
        current_token++;

        for (uint8_t i = 0; i < stage_length; i++) {
          parsed_length = (parsed_length * 10) + (_parsed_message[i] - '0');
        }

        parsing_stage = 2;
        stage_length = 0;

        memset(_parsed_message, '\0', FRAMEBUFFER_SIZE);
        continue;
      }

      _parsed_message[stage_length] = tok;
      stage_length += 1;
    }

    if (parsing_stage == 2 && parsed_length > 0) {
      log_d("parsed message - '%s'", _parsed_message);
      return 2;
    }

    return 1;
  }

  Manager::Connected::Connected(Preferences* _preferences):
    _authorization_stage(EAuthorizationStage::NotRequested),
    _cursor(0),
    _framebuffer((char*) malloc(sizeof(char) * FRAMEBUFFER_SIZE)),
    _outbound_buffer((char*) malloc(sizeof(char) * OUTBOUND_BUFFER_SIZE)),
    _parsed_message((char*) malloc(sizeof(char) * PARSED_MESSAGE_SIZE)),
    _device_id_len(0),
    _empty_identified_reads(0),
    _cached_reset_count(0),
    _preferences(_preferences) {
      _device_id = (char*) malloc(sizeof(char) * MAX_ID_SIZE);
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      memset(_device_id, '\0', MAX_ID_SIZE + 1);
  }

  // Called when we're in a `NotRequested` state, this method is responsible for opening our
  // tcp connection with the correct certificate and issuing our authorization message.
  std::optional<Manager::EManagerMessage> Manager::Connected::connect(
    const char * _redis_host,
    const std::pair<const char *, const char *> & _redis_auth,
    uint32_t _redis_port
  ) {
    log_d("attempting to certify redis connection");

    // TODO: figure out how to maybe read this elsewhere to make this library more portable?
    extern const uint8_t redis_root_ca[] asm("_binary_embeds_redis_host_root_ca_pem_start");

    _client.setCACert((char *) redis_root_ca);
    log_d("attempting to establish connection with redis %s:%d", _redis_host, _redis_port);

    int result = _client.connect(_redis_host, _redis_port);

    if (result != 1) {
      log_e("unable to establish connection - %d", result);
      return Manager::EManagerMessage::FailedConnection;
    }

    size_t stored_id_len = _preferences->isKey("device-id")
      ?  _preferences->getString("device-id", _device_id, MAX_ID_SIZE)
      : 0;

    // If we have a stored device id in our preferences/persistent memory, attempt to authorize
    // with it and move into the authorization requested stage.
    if (stored_id_len > 0) {
      _connected_with_cached_id = true;
      _device_id_len = stored_id_len;

      log_d("has stored device id '%s', trying it out.", _device_id);

      // Now that we have a valid device id, authorize as that.
      memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
      sprintf(
        _outbound_buffer,
        "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
        strlen(_device_id),
        _device_id,
        strlen(_device_id),
        _device_id
      );
      _client.print(_outbound_buffer);
      memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);

      // Here we are skipping right over `AuthorizationInitiated`; we had a device id in our
      // cache and attempted to use it.
      _authorization_stage = EAuthorizationStage::AuthorizationAttempted;

      return Manager::EManagerMessage::IdentificationReceived;
    }

    // So we've connected without stored credentials. What we'll do now is attempt to authenticate
    // with the burned-in ACL information which should only have the ability to pop an id.
    _authorization_stage = EAuthorizationStage::AuthorizationRequested;

    // At this point we have a valid tcp connection using tls and can start writing our redis
    // commands. Start by sending the `AUTH` command using the device id consumer ACL that should
    // be safe for global use.
    memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
    const char * redis_username = std::get<0>(_redis_auth);
    const char * redis_password = std::get<1>(_redis_auth);

    sprintf(
      _outbound_buffer,
      "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
      strlen(redis_username),
      redis_username,
      strlen(redis_password),
      redis_password
    );

    // TODO: `_client.write(...)` might be a better choice here
    size_t written = _client.print(_outbound_buffer);
    log_d("requested authenicated session using burn-in credentials (%d bytes)", written);

    return std::nullopt;
  }

  void Manager::Connected::reset(void) {
    _strange_thing_count = 0;
    _empty_identified_reads = 0;
    _cached_reset_count += 1;
    _pending_response = false;

    _authorization_stage = EAuthorizationStage::NotRequested;
    _client.stop();

    if (_cached_reset_count > MAX_RESETS_RECREDENTIALIZE) {
      _cached_reset_count = 0;
      log_e("client resets without successfull message exceeded max; removing device id");
      _preferences->remove("device-id");
    }

    _cursor = 0;
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
  }

  Manager::Connected::~Connected() {
    log_d("cleaning up redis client connection");

    free(_framebuffer);
    free(_parsed_message);
    free(_device_id);
    free(_outbound_buffer);

    _client.stop();
  }

  inline uint16_t Manager::Connected::write_message(uint32_t current_time) {
    if (_write_timer.update(current_time) != 1) {
      return 0;
    }

    _pending_response = true;
    memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
    if (_last_written_pop == false) {
      log_i("writing receiving pop");
      _last_written_pop = true;
      uint8_t keysize = strlen(_device_id) + 3;
      sprintf(_outbound_buffer, "*2\r\n$4\r\nLPOP\r\n$%d\r\nob:%s\r\n", keysize, _device_id);
    } else {
      log_i("writing diagnostic push");
      _last_written_pop = false;
      sprintf(_outbound_buffer, "*3\r\n$5\r\nRPUSH\r\n$4\r\nob:i\r\n$%d\r\n%s\r\n", strlen(_device_id), _device_id);
    }

    auto bytes_sent = _client.print(_outbound_buffer);
    memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
    return bytes_sent;
  }

  bool Manager::Disconnected::update(std::optional<wifimanager::Manager::EManagerMessage> &message) {
    return message == wifimanager::Manager::EManagerMessage::Connected;
  }

  uint8_t Manager::id_size(void) {
    Connected * c = std::get_if<Connected>(&_state);
    if (!c) {
      return 0;
    }
    return c->_device_id_len;
  }

  uint8_t Manager::copy_id(char * dest, uint8_t max) {
    Connected * c = std::get_if<Connected>(&_state);
    if (!c) {
      return 0;
    }
    uint8_t amount = c->_device_id_len < max ? c->_device_id_len : max;
    memcpy(dest, c->_device_id, amount);
    return amount;
  }
}
