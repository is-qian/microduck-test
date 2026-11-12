#ifndef VL53L8CX_PLATFORM_LINUX_H
#define VL53L8CX_PLATFORM_LINUX_H

#include "vl53l8cx_platform.h"

typedef struct {
	int fd;
	uint32_t speed_hz;
} vl53l8cx_spi_t;

void vl53l8cx_platform_init(VL53L8CX_Platform *platform, vl53l8cx_spi_t *spi);

#endif
