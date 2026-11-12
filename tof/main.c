/*
 * VL53L8CX ranging test for the MicroDuck Face Board (SPI3, 1V8 domain).
 * Usage: vl53l8cx-test [spidev] [frames] [resolution 4|8] [freq_hz]
 */

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <linux/spi/spidev.h>

#include "vl53l8cx_api.h"
#include "platform_linux.h"

static int open_spidev(const char *path, uint32_t speed)
{
	uint8_t mode = SPI_MODE_3, bits = 8;
	int fd = open(path, O_RDWR);

	if (fd < 0)
		return -1;
	if (ioctl(fd, SPI_IOC_WR_MODE, &mode) < 0 ||
	    ioctl(fd, SPI_IOC_WR_BITS_PER_WORD, &bits) < 0 ||
	    ioctl(fd, SPI_IOC_WR_MAX_SPEED_HZ, &speed) < 0) {
		close(fd);
		return -1;
	}
	return fd;
}

int main(int argc, char **argv)
{
	const char *path = argc > 1 ? argv[1] : "/dev/spidev3.0";
	int frames = argc > 2 ? atoi(argv[2]) : 20;
	int res = argc > 3 ? atoi(argv[3]) : 8;
	int freq = argc > 4 ? atoi(argv[4]) : (res == 4 ? 60 : 15);
	/* 8 MHz corrupts transfers on the FPC link (torn reads, failed fw upload) */
	vl53l8cx_spi_t spi = { .speed_hz = 4000000 };
	VL53L8CX_Configuration dev;
	VL53L8CX_ResultsData results;
	uint8_t alive, ready, status;
	int side, i, j;
	struct timespec t0, t1;
	int bad_frames = 0, valid_zones = 0, zones;

	spi.fd = open_spidev(path, spi.speed_hz);
	if (spi.fd < 0) {
		perror(path);
		return 1;
	}

	dev.platform.address = 0;
	vl53l8cx_platform_init(&dev.platform, &spi);

	usleep(10000);
	status = vl53l8cx_is_alive(&dev, &alive);
	if (status || !alive) {
		fprintf(stderr, "sensor not alive (status=%u alive=%u)\n", status, alive);
		return 1;
	}
	printf("VL53L8CX detected on %s\n", path);

	status = vl53l8cx_init(&dev);
	if (status) {
		fprintf(stderr, "init failed: %u\n", status);
		return 1;
	}
	printf("firmware loaded, init ok\n");

	vl53l8cx_set_resolution(&dev, res == 4 ? VL53L8CX_RESOLUTION_4X4
					       : VL53L8CX_RESOLUTION_8X8);
	if (vl53l8cx_set_ranging_frequency_hz(&dev, (uint8_t)freq)) {
		fprintf(stderr, "set frequency %dHz failed\n", freq);
		return 1;
	}
	vl53l8cx_get_ranging_frequency_hz(&dev, &alive);
	printf("frequency readback: %u Hz\n", alive);
	vl53l8cx_start_ranging(&dev);

	side = res == 4 ? 4 : 8;
	clock_gettime(CLOCK_MONOTONIC, &t0);
	for (i = 0; i < frames; i++) {
		ready = 0;
		for (j = 0; j < 100 && !ready; j++) {
			vl53l8cx_check_data_ready(&dev, &ready);
			if (!ready)
				usleep(1000);
		}
		if (!ready) {
			fprintf(stderr, "frame %d: timeout\n", i);
			break;
		}
		vl53l8cx_get_ranging_data(&dev, &results);
		zones = 0;
		for (j = 0; j < side * side; j++)
			if (results.target_status[VL53L8CX_NB_TARGET_PER_ZONE * j] == 5 ||
			    results.target_status[VL53L8CX_NB_TARGET_PER_ZONE * j] == 6 ||
			    results.target_status[VL53L8CX_NB_TARGET_PER_ZONE * j] == 9)
				zones++;
		if (zones < side * side / 2)
			bad_frames++;
		valid_zones += zones;
		if (frames <= 20 || i == frames - 1) {
			printf("frame %2d distance_mm:\n", i);
			for (j = 0; j < side * side; j++) {
				printf("%5d", results.distance_mm[VL53L8CX_NB_TARGET_PER_ZONE * j]);
				if (j % side == side - 1)
					printf("\n");
			}
		}
	}
	clock_gettime(CLOCK_MONOTONIC, &t1);

	vl53l8cx_stop_ranging(&dev);
	close(spi.fd);
	if (i > 1) {
		double dt = (t1.tv_sec - t0.tv_sec) + (t1.tv_nsec - t0.tv_nsec) / 1e9;
		printf("%d frames in %.3fs -> %.2f fps (target %dHz)\n",
		       i, dt, (i - 1) / dt, freq);
		printf("data check: avg valid zones %.1f/%d, %d frames mostly invalid\n",
		       (double)valid_zones / i, side * side, bad_frames);
	}
	return 0;
}
