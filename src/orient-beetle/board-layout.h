#ifndef _BOARD_LAYOUT_H
#define _BOARD_LAYOUT_H

/**
 * Orient Display Pinout:
#define PIN_TFT_RESET 4
#define PIN_TFT_DC    16
#define PIN_TFT_CS    17
#define PIN_TFT_BL 13
 */

/**
 * DFRobot Display Pinout (GDI Cable) */
#define PIN_TFT_RESET 26
#define PIN_TFT_DC    25
#define PIN_TFT_CS    14
#define PIN_TFT_BL    12
/* */

#ifndef LED_BUILTIN
#define LED_BUILTIN 2
#endif

//  A4 15 - check
//  A3 35 - no check
//  A2 00 - no check
//  A1 00 - no check
//  A0 00 - no check
// D13 12 - no check
// D12 04 - check
// D11 16 - check
// D10 17 - check

// D2 25 - check
// D3 26 - check
// D5 00 - no check
// D6 14 - check
// D7 13 - check
// D9 02 - check


#endif
