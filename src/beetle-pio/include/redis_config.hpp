#ifndef _REDIS_CONFIG_H
#define _REDIS_CONFIG_H 1

#ifdef REDIS_HOST
const char * redis_host = "" REDIS_HOST;
#endif

#ifdef REDIS_PORT
const uint32_t redis_port = REDIS_PORT;
#endif

#ifdef REDIS_AUTH
const char * redis_auth = REDIS_AUTH;
#endif

#endif
