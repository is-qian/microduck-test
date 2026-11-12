/*
 * VL53L8CX platform layer for Linux spidev.
 * SPI mode 3, MSB first; write = addr | 0x8000, read = addr & ~0x8000.
 */

#include <stdint.h>
#include <string.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <linux/spi/spidev.h>

#include "platform_linux.h"

#define CHUNK 2048

static int spi_xfer(int fd, uint8_t *tx, uint8_t *rx, uint32_t len, uint32_t speed)
{
	struct spi_ioc_transfer tr = {
		.tx_buf = (unsigned long)tx,
		.rx_buf = (unsigned long)rx,
		.len = len,
		.speed_hz = speed,
		.bits_per_word = 8,
	};
	return ioctl(fd, SPI_IOC_MESSAGE(1), &tr);
}

static uint8_t platform_write(void *handle, uint16_t reg, uint8_t *data, uint32_t size)
{
	vl53l8cx_spi_t *spi = handle;
	uint8_t buf[CHUNK + 2];
	uint32_t pos;

	for (pos = 0; pos < size; pos += CHUNK) {
		uint32_t n = size - pos > CHUNK ? CHUNK : size - pos;
		uint16_t addr = (reg + pos) | 0x8000;
		buf[0] = addr >> 8;
		buf[1] = addr & 0xFF;
		memcpy(buf + 2, data + pos, n);
		if (spi_xfer(spi->fd, buf, NULL, n + 2, spi->speed_hz) < 0)
			return 1;
	}
	return 0;
}

static uint8_t platform_read(void *handle, uint16_t reg, uint8_t *data, uint32_t size)
{
	vl53l8cx_spi_t *spi = handle;
	uint8_t buf[CHUNK + 2];
	uint32_t pos;

	for (pos = 0; pos < size; pos += CHUNK) {
		uint32_t n = size - pos > CHUNK ? CHUNK : size - pos;
		uint16_t addr = (reg + pos) & 0x7FFF;
		memset(buf, 0, n + 2);
		buf[0] = addr >> 8;
		buf[1] = addr & 0xFF;
		if (spi_xfer(spi->fd, buf, buf, n + 2, spi->speed_hz) < 0)
			return 1;
		memcpy(data + pos, buf + 2, n);
	}
	return 0;
}

static uint8_t platform_wait(void *handle, uint32_t ms)
{
	(void)handle;
	usleep(ms * 1000);
	return 0;
}

void vl53l8cx_platform_init(VL53L8CX_Platform *platform, vl53l8cx_spi_t *spi)
{
	platform->address = 0;
	platform->handle = spi;
	platform->Write = platform_write;
	platform->Read = platform_read;
	platform->Wait = platform_wait;
}
