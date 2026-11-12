/* RFAL platform layer for Linux: spidev + gpiochip (v1 API), polling IRQ */
#include "rfal_platform.h"

#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <sys/ioctl.h>
#include <linux/spi/spidev.h>
#include <linux/gpio.h>

#define NFC_GPIO_CHIP   "/dev/gpiochip3"
#define NFC_IRQ_LINE    20   /* GPIO3_C4 */
#define NFC_RESET_LINE  21   /* GPIO3_C5, active high */
#define NFC_SPIDEV      "/dev/spidev1.0"
#define NFC_SPI_SPEED   2000000

static int spi_fd = -1;
static int irq_fd = -1;
static int reset_fd = -1;
static int dbg = -1;

static int debug_on(void)
{
    if (dbg < 0)
        dbg = getenv("NFC_DEBUG") ? 1 : 0;
    return dbg;
}

int rfal_platform_init(void)
{
    int chip = open(NFC_GPIO_CHIP, O_RDONLY);
    if (chip < 0) { perror(NFC_GPIO_CHIP); return -1; }

    struct gpiohandle_request req;
    memset(&req, 0, sizeof(req));
    req.lineoffsets[0] = NFC_IRQ_LINE;
    req.lines = 1;
    req.flags = GPIOHANDLE_REQUEST_INPUT;
    strcpy(req.consumer_label, "st25r-irq");
    if (ioctl(chip, GPIO_GET_LINEHANDLE_IOCTL, &req) < 0) { perror("irq line"); close(chip); return -1; }
    irq_fd = req.fd;

    memset(&req, 0, sizeof(req));
    req.lineoffsets[0] = NFC_RESET_LINE;
    req.lines = 1;
    req.flags = GPIOHANDLE_REQUEST_OUTPUT;
    strcpy(req.consumer_label, "st25r-reset");
    req.default_values[0] = 1; /* hold in reset until RFAL init */
    req.fd = -1;
    if (ioctl(chip, GPIO_GET_LINEHANDLE_IOCTL, &req) < 0) {
        if (errno != EBUSY) { perror("reset line"); close(chip); return -1; }
        /* yocto dt hogs the line low (reset released); chip runs without us */
        fprintf(stderr, "reset line busy (dt hog), skipping hard reset\n");
    } else {
        reset_fd = req.fd;
    }
    close(chip);

    spi_fd = open(NFC_SPIDEV, O_RDWR);
    if (spi_fd < 0) { perror(NFC_SPIDEV); return -1; }
    unsigned char mode = SPI_MODE_1, bits = 8;
    unsigned int speed = NFC_SPI_SPEED;
    ioctl(spi_fd, SPI_IOC_WR_MODE, &mode);
    ioctl(spi_fd, SPI_IOC_WR_BITS_PER_WORD, &bits);
    ioctl(spi_fd, SPI_IOC_WR_MAX_SPEED_HZ, &speed);
    return 0;
}

static void gpio_write(int fd, uint8_t val)
{
    if (fd < 0) return;
    struct gpiohandle_data data;
    memset(&data, 0, sizeof(data));
    data.values[0] = val;
    ioctl(fd, GPIOHANDLE_SET_LINE_VALUES_IOCTL, &data);
}

static uint8_t gpio_read(int fd)
{
    struct gpiohandle_data data;
    memset(&data, 0, sizeof(data));
    if (ioctl(fd, GPIOHANDLE_GET_LINE_VALUES_IOCTL, &data) < 0)
        return 0;
    return data.values[0];
}

void rfal_platform_gpio_set(uint32_t pin)   { (void)pin; gpio_write(reset_fd, 1); }
void rfal_platform_gpio_clear(uint32_t pin) { (void)pin; gpio_write(reset_fd, 0); }

bool rfal_platform_gpio_is_high(uint32_t pin)
{
    (void)pin;
    return gpio_read(irq_fd) != 0;
}

static uint32_t now_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)(ts.tv_sec * 1000u + ts.tv_nsec / 1000000u);
}

uint32_t rfal_platform_timer_create(uint32_t ms)        { return now_ms() + ms; }
bool rfal_platform_timer_is_expired(uint32_t timer)     { return (int32_t)(now_ms() - timer) >= 0; }
uint32_t rfal_platform_timer_get_remaining(uint32_t timer)
{
    int32_t r = (int32_t)(timer - now_ms());
    return r > 0 ? (uint32_t)r : 0;
}

void rfal_platform_delay(uint32_t ms) { usleep(ms * 1000); }
uint32_t rfal_platform_systick(void)  { return now_ms(); }

int rfal_platform_spi_txrx(const uint8_t *tx, uint8_t *rx, uint16_t len)
{
    if (debug_on() && tx) {
        fprintf(stderr, "spi %3u >>", len);
        for (uint16_t i = 0; i < len && i < 12; i++)
            fprintf(stderr, " %02X", tx[i]);
        fprintf(stderr, "\n");
    }
    struct spi_ioc_transfer tr;
    memset(&tr, 0, sizeof(tr));
    tr.tx_buf = (unsigned long)tx;
    tr.rx_buf = (unsigned long)rx;
    tr.len = len;
    tr.speed_hz = NFC_SPI_SPEED;
    tr.bits_per_word = 8;
    if (ioctl(spi_fd, SPI_IOC_MESSAGE(1), &tr) < 0) {
        perror("spi txrx");
        return -1;
    }
    if (debug_on() && rx) {
        fprintf(stderr, "spi %3u <<", len);
        for (uint16_t i = 0; i < len && i < 12; i++)
            fprintf(stderr, " %02X", rx[i]);
        fprintf(stderr, "\n");
    }
    return 0;
}

/* direct register write for test hooks (e.g. antenna selection) */
void rfal_platform_write_reg(uint8_t addr, uint8_t val)
{
    uint8_t b[2] = { addr, val };
    rfal_platform_spi_txrx(b, b, 2);
}
