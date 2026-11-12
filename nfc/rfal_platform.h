/* RFAL platform layer for Linux (spidev + gpiochip), ST25R100/200 polling IRQ mode */
#pragma once
#ifndef RFAL_PLATFORM_H
#define RFAL_PLATFORM_H

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define RFAL_USE_I2C 0
#define ST25R_COM_SINGLETXRX
#define ST25R_POLL_IRQ
#define ST25R_IRQ_POLL_REGS   /* IRQ pin unusable on MicroDuck D4; poll IRQ status regs via SPI */

#define RFAL_FEATURE_LISTEN_MODE      false
#define RFAL_FEATURE_WAKEUP_MODE      false
#define RFAL_FEATURE_LOWPOWER_MODE    false
#define RFAL_FEATURE_NFCA             true
#define RFAL_FEATURE_NFCB             true
#define RFAL_FEATURE_NFCF             false
#define RFAL_FEATURE_NFCV             true
#define RFAL_FEATURE_T1T              false
#define RFAL_FEATURE_T2T              true
#define RFAL_FEATURE_T4T              false
#define RFAL_FEATURE_ST25TB           true
#define RFAL_FEATURE_ST25xV           false
#define RFAL_FEATURE_DPO              false
#define RFAL_FEATURE_ISO_DEP          true
#define RFAL_FEATURE_ISO_DEP_POLL     true
#define RFAL_FEATURE_ISO_DEP_LISTEN   false
#define RFAL_FEATURE_NFC_DEP          false
#define RFAL_FEATURE_NFC_DEP_POLL     false
#define RFAL_FEATURE_NFC_DEP_LISTEN   false

#define RFAL_FEATURE_ISO_DEP_IBLOCK_MAX_LEN  256U
#define RFAL_FEATURE_ISO_DEP_APDU_MAX_LEN    512U
#define RFAL_FEATURE_NFC_DEP_BLOCK_MAX_LEN   254U
#define RFAL_FEATURE_NFC_DEP_PDU_MAX_LEN     512U
#define RFAL_FEATURE_NFC_RF_BUF_LEN          258U

#define ST25R_INT_PORT    0
#define ST25R_INT_PIN     0
#define ST25R_RESET_PORT  0
#define ST25R_RESET_PIN   0

#define PLATFORM_LED_CONN_PIN 1U
#define PLATFORM_LED_STAT_PIN 0U
#define PLATFORM_LED_RX_PIN   2U
#define PLATFORM_LED_TX_PIN   3U

int      rfal_platform_init(void);
void     rfal_platform_gpio_set(uint32_t pin);
void     rfal_platform_gpio_clear(uint32_t pin);
bool     rfal_platform_gpio_is_high(uint32_t pin);
uint32_t rfal_platform_timer_create(uint32_t ms);
bool     rfal_platform_timer_is_expired(uint32_t timer);
uint32_t rfal_platform_timer_get_remaining(uint32_t timer);
void     rfal_platform_delay(uint32_t ms);
uint32_t rfal_platform_systick(void);
int      rfal_platform_spi_txrx(const uint8_t *tx, uint8_t *rx, uint16_t len);

#define platformProtectST25RComm()        ((void)0)
#define platformUnprotectST25RComm()      ((void)0)
#define platformProtectST25RIrqStatus()   ((void)0)
#define platformUnprotectST25RIrqStatus() ((void)0)
#define platformProtectWorker()           ((void)0)
#define platformUnprotectWorker()         ((void)0)

#define platformIrqST25RPinInitialize()   ((void)0)
#define platformIrqST25RSetCallback(cb)   ((void)0)

#define platformLedsInitialize()          ((void)0)
#define platformLedOff(port, pin)         ((void)0)
#define platformLedOn(port, pin)          ((void)0)
#define platformLedToggle(port, pin)      ((void)0)

#define platformGpioSet(port, pin)        rfal_platform_gpio_set(pin)
#define platformGpioClear(port, pin)      rfal_platform_gpio_clear(pin)
#define platformGpioIsHigh(port, pin)     rfal_platform_gpio_is_high(pin)
#define platformGpioIsLow(port, pin)      (!rfal_platform_gpio_is_high(pin))

#define platformTimerCreate(t)            rfal_platform_timer_create(t)
#define platformTimerIsExpired(t)         rfal_platform_timer_is_expired(t)
#define platformTimerDestroy(t)           ((void)0)
#define platformTimerGetRemaining(t)      rfal_platform_timer_get_remaining(t)

#define platformDelay(t)                  rfal_platform_delay(t)
#define platformGetSysTick()              rfal_platform_systick()

#define platformErrorHandle()             ((void)0)

#define platformSpiSelect()               ((void)0)
#define platformSpiDeselect()             ((void)0)
#define platformSpiTxRx(tx, rx, len)      rfal_platform_spi_txrx(tx, rx, len)

#define platformLog(...)                  ((void)0)
#ifndef platformAssert
#define platformAssert(exp)               ((void)(exp))
#endif
#define platformFlushCache()              ((void)0)

#ifdef __cplusplus
}
#endif

#endif /* RFAL_PLATFORM_H */
