CC ?= gcc
CFLAGS += -O2 -Wall

vl53l8cx-test: main.c platform_linux.c vl53l8cx_api.c vl53l8cx_platform.c
	$(CC) $(CFLAGS) -o $@ $^

clean:
	rm -f vl53l8cx-test

.PHONY: clean
