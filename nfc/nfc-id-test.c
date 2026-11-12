/*
 * ST25R100 smoke test: release reset and read IC_ID register over spidev.
 * Read opcode is addr|0x80, IC_ID register is 0x3F, expected value 0xA9.
 */
#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <sys/ioctl.h>
#include <linux/spi/spidev.h>
#include <linux/gpio.h>

#define NFC_RESET_LINE 21 /* GPIO3_C5 on gpiochip3, reset is active high */

int main(int argc, char **argv)
{
    const char *spidev = argc > 1 ? argv[1] : "/dev/spidev1.0";

    int gfd = open("/dev/gpiochip3", O_RDONLY);
    struct gpiohandle_request req;
    struct gpiohandle_data data;
    memset(&req, 0, sizeof(req));
    req.lineoffsets[0] = NFC_RESET_LINE;
    req.lines = 1;
    req.flags = GPIOHANDLE_REQUEST_OUTPUT;
    strcpy(req.consumer_label, "nfc-id");
    req.default_values[0] = 0;
    req.fd = -1;
    if (ioctl(gfd, GPIO_GET_LINEHANDLE_IOCTL, &req) < 0)
        perror("gpiochip3");
    data.values[0] = 0;
    if (req.fd >= 0)
        ioctl(req.fd, GPIOHANDLE_SET_LINE_VALUES_IOCTL, &data);
    usleep(10000);

    int sfd = open(spidev, O_RDWR);
    if (sfd < 0) { perror(spidev); return 1; }
    unsigned char mode = SPI_MODE_1, bits = 8;
    unsigned int speed = 2000000;
    ioctl(sfd, SPI_IOC_WR_MODE, &mode);
    ioctl(sfd, SPI_IOC_WR_BITS_PER_WORD, &bits);
    ioctl(sfd, SPI_IOC_WR_MAX_SPEED_HZ, &speed);

    unsigned char tx[3] = { 0x3F | 0x80, 0x00, 0x00 }, rx[3] = { 0 };
    struct spi_ioc_transfer tr;
    memset(&tr, 0, sizeof(tr));
    tr.tx_buf = (unsigned long)tx;
    tr.rx_buf = (unsigned long)rx;
    tr.len = 3;
    tr.speed_hz = speed;
    tr.bits_per_word = bits;
    if (ioctl(sfd, SPI_IOC_MESSAGE(1), &tr) < 0) { perror("xfer"); return 1; }

    printf("IC_ID = 0x%02X (expect 0xA9)\n", rx[1]);
    return rx[1] == 0xA9 ? 0 : 2;
}
