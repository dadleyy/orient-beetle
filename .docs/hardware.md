### Rev. 2

| name | manufacturer | price | docs |
| ---- | ---- | ---- | --- |
| Xiao ESP32C3 | Seeed Studios | [~$5 (@ digikey.com)][xiao-digi] | [wiki][xiao-wiki] |
| 4.2" E-Ink Display | Orient Displays | [~$35 (@ amazon.com)][xiao-display-amzn] | |

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
