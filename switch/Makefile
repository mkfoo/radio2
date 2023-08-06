PROGRAM = switchmon
VPATH = src
CC = clang
CFLAGS = -Werror -Wall -Wextra -Wpedantic -std=c17 -D_POSIX_C_SOURCE=199309L \
		 --sysroot=/usr/aarch64-suse-linux/sys-root --target=aarch64-unknown-linux-gnu -mcpu=cortex-a53
OFLAGS = -O1
OBJECTS = main.o
HEADERS = switchmon.h

$(PROGRAM) : $(OBJECTS) 
	$(CC) $(CFLAGS) $(OFLAGS) $(OBJECTS) -o $(PROGRAM)

$(OBJECTS) : %.o: %.c $(HEADERS)
	$(CC) -c $(CFLAGS) $(OFLAGS) $< -o $@

format :
	clang-format -i src/*

clean :
	rm -f $(PROGRAM) *.o

.PHONY : format clean
