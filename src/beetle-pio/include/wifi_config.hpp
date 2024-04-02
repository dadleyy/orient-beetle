#ifndef _WIFI_CONFIG_H
#define _WIFI_CONFIG_H 1

#ifndef WIFI_SSID
#define WIFI_SSID "orient-beetle setup"
#endif

#ifndef WIFI_PASSWORD
// This is the default-wifi password that the device will use to establish a
// network that users can connect to and provide their SSID credentials via the
// captive portal.
#define WIFI_PASSWORD "orientbeetle"
#endif

const char* ap_ssid = WIFI_SSID;
const char* ap_password = WIFI_PASSWORD;

#endif
