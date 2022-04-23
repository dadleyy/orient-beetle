#include <Arduino.h>
#include <SPI.h>

SPIClass SPI3(VSPI);

void SPI_WriteCom(byte byInst, unsigned int cs, unsigned int dc) {
  digitalWrite(cs, LOW);
  digitalWrite(dc, LOW);
  SPI3.transfer(byInst);
  digitalWrite(cs, HIGH);
}

void SPI_WriteData(word byData, unsigned int cs, unsigned int dc) {
  digitalWrite(cs, LOW);
  digitalWrite(dc, HIGH);
  SPI3.transfer(byData);
  digitalWrite(cs, HIGH);
}

void LCD_Init(unsigned int cs, unsigned int dc) {
  SPI_WriteCom(0xCF, cs, dc);
  SPI_WriteData(0x00, cs, dc);
  SPI_WriteData(0xCB, cs, dc);
  SPI_WriteData(0X30, cs, dc);

  SPI_WriteCom(0xED, cs, dc);
  SPI_WriteData(0x64, cs, dc);
  SPI_WriteData(0x03, cs, dc);
  SPI_WriteData(0X12, cs, dc);
  SPI_WriteData(0X81, cs, dc);

  SPI_WriteCom(0xE8, cs, dc);
  SPI_WriteData(0x85, cs, dc);
  SPI_WriteData(0x10, cs, dc);
  SPI_WriteData(0x7A, cs, dc);

  SPI_WriteCom(0xCB, cs, dc);
  SPI_WriteData(0x39, cs, dc);
  SPI_WriteData(0x2C, cs, dc);
  SPI_WriteData(0x00, cs, dc);
  SPI_WriteData(0x34, cs, dc);
  SPI_WriteData(0x02, cs, dc);

  SPI_WriteCom(0xF7, cs, dc);
  SPI_WriteData(0x20, cs, dc);

  SPI_WriteCom(0xEA, cs, dc);
  SPI_WriteData(0x00, cs, dc);
  SPI_WriteData(0x00, cs, dc);

  SPI_WriteCom(0xC0, cs, dc);    //Power control
  SPI_WriteData(0x21, cs, dc);   //VRH[5:0]

  SPI_WriteCom(0xC1, cs, dc);    //Power control
  SPI_WriteData(0x11, cs, dc);   //SAP[2:0];BT[3:0]

  SPI_WriteCom(0xC5, cs, dc);    //VCM control
  SPI_WriteData(0x3F, cs, dc);
  SPI_WriteData(0x3C, cs, dc);

  SPI_WriteCom(0xC7, cs, dc);    //VCM control2
  SPI_WriteData(0XAF, cs, dc);

  SPI_WriteCom(0x36, cs, dc);    // Memory Access Control
  SPI_WriteData(0x08, cs, dc);

  SPI_WriteCom(0x3A, cs, dc);
  SPI_WriteData(0x55, cs, dc);

  SPI_WriteCom(0xB1, cs, dc);
  SPI_WriteData(0x00, cs, dc);
  SPI_WriteData(0x1B, cs, dc);

  SPI_WriteCom(0xB6, cs, dc);    // Display Function Control
  SPI_WriteData(0x0A, cs, dc);
  SPI_WriteData(0xA2, cs, dc);

  SPI_WriteCom(0xF2, cs, dc);    // 3Gamma Function Disable
  SPI_WriteData(0x00, cs, dc);

  SPI_WriteCom(0x26, cs, dc);    //Gamma curve selected
  SPI_WriteData(0x01, cs, dc);

  SPI_WriteCom(0xE0, cs, dc);    //Set Gamma
  SPI_WriteData(0x0F, cs, dc);
  SPI_WriteData(0x23, cs, dc);
  SPI_WriteData(0x20, cs, dc);
  SPI_WriteData(0x0C, cs, dc);
  SPI_WriteData(0x0F, cs, dc);
  SPI_WriteData(0x09, cs, dc);
  SPI_WriteData(0x4E, cs, dc);
  SPI_WriteData(0XA8, cs, dc);
  SPI_WriteData(0x3D, cs, dc);
  SPI_WriteData(0x0B, cs, dc);
  SPI_WriteData(0x15, cs, dc);
  SPI_WriteData(0x06, cs, dc);
  SPI_WriteData(0x0E, cs, dc);
  SPI_WriteData(0x08, cs, dc);
  SPI_WriteData(0x00, cs, dc);

  SPI_WriteCom(0XE1, cs, dc);
  SPI_WriteData(0x00, cs, dc);
  SPI_WriteData(0x1C, cs, dc);
  SPI_WriteData(0x1F, cs, dc);
  SPI_WriteData(0x03, cs, dc);
  SPI_WriteData(0x10, cs, dc);
  SPI_WriteData(0x06, cs, dc);
  SPI_WriteData(0x31, cs, dc);
  SPI_WriteData(0x57, cs, dc);
  SPI_WriteData(0x42, cs, dc);
  SPI_WriteData(0x04, cs, dc);
  SPI_WriteData(0x0A, cs, dc);
  SPI_WriteData(0x09, cs, dc);
  SPI_WriteData(0x31, cs, dc);
  SPI_WriteData(0x37, cs, dc);
  SPI_WriteData(0x0F, cs, dc);

  SPI_WriteCom(0x11, cs, dc);    //Exit Sleep
  delay(120);
  SPI_WriteCom(0x29, cs, dc);
  SPI_WriteCom(0xb0, cs, dc);
  SPI_WriteData(0x80, cs, dc);
}
