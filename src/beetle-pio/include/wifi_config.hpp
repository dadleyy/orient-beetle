#ifndef _WIFI_CONFIG_H
#define _WIFI_CONFIG_H 1

#ifndef WIFI_SSID
#define WIFI_SSID "orient-beetle setup"
#endif
#ifndef WIFI_PASSWORD
#define WIFI_PASSWORD "password"
#endif

const char * ap_ssid PROGMEM = WIFI_SSID;
const char * ap_password PROGMEM = WIFI_PASSWORD;

#endif
