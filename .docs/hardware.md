### Current Hardware

| name | manufacturer | price | docs |
| ---- | ---- | ---- | --- |
| Firebeetle 2 | DFRobot | [~$8 (@ digikey.com)][beetle] | [wiki][beetle-wiki] |
| 2.4" TFT Display | Orient Displays | [~$14 (@ digikey.com)][display] | [datasheet][display-sheet] |
| VCNL4010 Sensor | Adafruit | [~$8 (@ adafruit.com)][sensor] | [wiki][sensor-wiki] |

---

#### Pinout

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

[← README](./README.md)

[display]: https://www.digikey.com/en/products/detail/orient-display/AFL240320A0-2-4N12NTM-ANO/13916615
[beetle]: https://www.digikey.com/en/products/detail/dfrobot/DFR0654/13978504
[beetle-wiki]: https://wiki.dfrobot.com/FireBeetle_Board_ESP32_E_SKU_DFR0654
[display-sheet]: https://www.orientdisplay.com/wp-content/uploads/2021/02/AFL240320A0-2.4N12NTM-ANO.pdf
[sensor]: https://www.adafruit.com/product/466?gclid=Cj0KCQjwxtSSBhDYARIsAEn0thTdgTAfUmzJ4P-3cUcmiMZ7yCLfQAEeFUWLr1lYPQIZ9KT-6T3ph9IaAvo0EALw_wcB
[sensor-wiki]: https://learn.adafruit.com/using-vcnl4010-proximity-sensor
