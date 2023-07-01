### Rev. 2

| name | manufacturer | price | docs |
| ---- | ---- | ---- | --- |
| Xiao ESP32C3 | Seeed Studios | [~$5 (@ digikey.com)][xiao-digi] | [wiki][xiao-wiki] |
| 4.2" E-Ink Display | Orient Displays | [~$35 (@ amazon.com)][xiao-display-amzn] | |
| ws2812b led | | [~$20 (@ amazon.com)][led-amzn] (partial strip use) | |
| lipo battery | | [4 pack ~$40 (@ amazon.com)][lip-amzn] (~$10 each) |
| antenna | | [2 pack ~$9 (@ amazon.com)][antenna-amzn] (~$4.50 each) |

> Note: the rev. 2 pcb "shield" schematic and board files can be found in the [`/hardware`](../hardware)
> directory. These were created in Eagle.

#### Xiao ESP32-C3 Soldering

The xiao-esp32c3 does include a battery charge chip, but the connections are not exposed as pin
through holes; they are surface pads on the bottom of the chip, marked by the `BAT` label:

| :camera: |
| --- |
| ![image](https://user-images.githubusercontent.com/1545348/250297269-1ab3f9f1-d234-4c86-a933-49f4625fba99.png) |

To use the lipo battery, JST cables like [these (amazon link)][jst-amzn] can be soldered to them,
making sure to match the polarity correctly (typically, red to `+`, black to `-`).

---

### Rev. 1*

| name | manufacturer | price | docs |
| ---- | ---- | ---- | --- |
| Firebeetle 2 | DFRobot | [~$8 (@ digikey.com)][beetle] | [wiki][beetle-wiki] |
| 2.4" TFT Display | Orient Displays | [~$14 (@ digikey.com)][display] | [datasheet][display-sheet] |
| VCNL4010 Sensor | Adafruit | [~$8 (@ adafruit.com)][sensor] | [wiki][sensor-wiki] |

> The architecure was overhauled in May 2023 (`v0.1.0`), with the responsibility of rendering messages
> moved from the device into the `srv-*` related appications. The firmware has not been updated to 
> reflect these changes.

##### Rev 1: Pinout

| display pin | firebeetle pin | notes |
| ---- | --- | --- |
| GND | GND | |
| VIN | 3V3 | |
| SCL | SCK |  |
| SDA | MOSI | |
| RST | D12 | this is pin `4` when using `digitalWrite(...)` |
| DC | D11 | this is pin `16` when using `digitalWrite(...)` |
| CS | D10 | this is pin `7` when using `digitalWrite(...)` |
| BLK | D7 | this is pin `13` when using `digitalWrite(...)` |

---

[← README](./README.md)

[display]: https://www.digikey.com/en/products/detail/orient-display/AFL240320A0-2-4N12NTM-ANO/13916615
[beetle]: https://www.digikey.com/en/products/detail/dfrobot/DFR0654/13978504
[beetle-wiki]: https://wiki.dfrobot.com/FireBeetle_Board_ESP32_E_SKU_DFR0654
[display-sheet]: https://www.orientdisplay.com/wp-content/uploads/2021/02/AFL240320A0-2.4N12NTM-ANO.pdf
[sensor]: https://www.adafruit.com/product/466?gclid=Cj0KCQjwxtSSBhDYARIsAEn0thTdgTAfUmzJ4P-3cUcmiMZ7yCLfQAEeFUWLr1lYPQIZ9KT-6T3ph9IaAvo0EALw_wcB
[sensor-wiki]: https://learn.adafruit.com/using-vcnl4010-proximity-sensor
[xiao-digi]: https://www.digikey.com/en/products/detail/seeed-technology-co-ltd/113991054/16652880?s=N4IgTCBcDaIB4EsCGB7ABAUwM4AcDMYAxiALoC%2BQA
[xiao-wiki]: https://www.seeedstudio.com/Seeed-XIAO-ESP32C3-p-5431.html
[xiao-display-amzn]: https://www.amazon.com/dp/B074NR1SW2
[led-amzn]: https://www.amazon.com/gp/product/B088BPGMXB
[lip-amzn]: https://www.amazon.com/gp/product/B095YB8CJK
[antenna-amzn]: https://www.amazon.com/gp/product/B01KBU61S8
[jst-amzn]: https://www.amazon.com/dp/B07FP2FCYC
