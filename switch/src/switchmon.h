#include <errno.h>
#include <fcntl.h>
#include <linux/gpio.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#define DEFAULT_GPIO_DEV "/dev/gpiochip0"
#define DEFAULT_SOCK_PATH "/run/dqtt/sock"
#define DEBOUNCE_NS 750000000

static int swm_init(void);
static int swm_read(int fd);
static int swm_write(int sockfd, int val);
static int swm_poll(int fd);
static int swm_quit(int fd);
static void _sleep(long nsec);
static unsigned int _get_pin_cfg(unsigned int lines, char* var);

static void _sleep(long nsec) {
    struct timespec tsp = {
        .tv_sec = 0,
        .tv_nsec = nsec,
    };
    int ret = 1;

    while (ret) {
        ret = nanosleep(&tsp, &tsp);
    }
}

static unsigned int _get_pin_cfg(unsigned int lines, char* var) {
    char* str = getenv(var);

    if (str == NULL) {
        fprintf(stderr, "Config variable %s not set\n", var);
        return 0;
    }

    int sval = atoi(str);

    if (sval < 1) {
        fprintf(stderr, "Invalid GPIO number %d\n", sval);
        return 0;
    }

    unsigned int val = (unsigned int)sval;

    if (val >= lines) {
        fprintf(stderr, "GPIO number %d out of range\n", val);
        return 0;
    }

    return val;
}

static int swm_init(void) {
    char* dev_path = getenv("SWM_CFG_GPIO_DEV");

    if (dev_path == NULL) {
        dev_path = DEFAULT_GPIO_DEV;
    }

    int fd = open(dev_path, O_RDONLY);

    if (fd < 0) {
        fprintf(stderr, "Could not open GPIO chip: %s\n", strerror(errno));
        return -1;
    }

    struct gpiochip_info chip_info;
    int ret = ioctl(fd, GPIO_GET_CHIPINFO_IOCTL, &chip_info);

    if (ret == -1) {
        fprintf(stderr, "Could not get GPIO chip info: %s\n", strerror(errno));
        return -1;
    }

    printf("Opened %s\n", chip_info.name);

    unsigned int lines = chip_info.lines;
    unsigned int a = _get_pin_cfg(lines, "SWM_CFG_A");
    unsigned int b = _get_pin_cfg(lines, "SWM_CFG_B");
    unsigned int c = _get_pin_cfg(lines, "SWM_CFG_C");

    if (!(a && b && c)) {
        return -1;
    }

    struct gpio_v2_line_request req = {
        .offsets = {a, b, c},
        .consumer = "switch monitor",
        .config =
            (struct gpio_v2_line_config){
                .flags = GPIO_V2_LINE_FLAG_INPUT |
                         GPIO_V2_LINE_FLAG_BIAS_PULL_UP |
                         GPIO_V2_LINE_FLAG_EDGE_RISING |
                         GPIO_V2_LINE_FLAG_EDGE_FALLING,
                .num_attrs = 0,
            },
        .num_lines = 3,
        .event_buffer_size = 12,
    };

    ret = ioctl(fd, GPIO_V2_GET_LINE_IOCTL, &req);

    if (ret == -1 || req.fd <= 0) {
        fprintf(stderr, "GPIO line request failed: %s\n", strerror(errno));
        return -1;
    }

    close(fd);

    int rfd = req.fd;
    int flags = fcntl(rfd, F_GETFL);

    if (flags < 0) {
        fprintf(stderr, "Could not get flags: %s\n", strerror(errno));
        return -1;
    }

    ret = fcntl(rfd, F_SETFL, flags | O_NONBLOCK);

    if (ret == -1) {
        fprintf(stderr, "fcntl failed: %s\n", strerror(errno));
        return -1;
    }

    return rfd;
}

static int swm_read(int fd) {
    _sleep(DEBOUNCE_NS);

    struct gpio_v2_line_event event;
    int ret = 1;

    while (ret > 0) {
        ret = read(fd, &event, sizeof(struct gpio_v2_line_event));
    }

    if (ret < 0 && errno != EAGAIN) {
        fprintf(stderr, "read failed: %s\n", strerror(errno));
        return -1;
    }

    struct gpio_v2_line_values data = {
        .bits = 0,
        .mask = 7,
    };

    ret = ioctl(fd, GPIO_V2_LINE_GET_VALUES_IOCTL, &data);

    if (ret == -1) {
        fprintf(stderr, "Failed to get GPIO values: %s\n", strerror(errno));
        return -1;
    }

    switch (data.bits) {
        case 1:
            return '0';
        case 3:
            return '1';
        case 2:
            return '2';
        case 6:
            return '3';
        default:
            return -2;
    }
}

static int swm_write(int sockfd, int val) {
    char msg[] = "\002switch\003channel=0\004";
    ssize_t len = 18;
    ssize_t written = 0;
    ssize_t ret = 0;
    msg[16] = val;

    while (written < len) {
        ret = write(sockfd, msg + written, len - written);

        if (ret < 0) {
            fprintf(stderr, "write failed: %s\n", strerror(errno));
            return -1;
        }

        written += ret;
    }

    return 0;
}

static int swm_poll(int fd) {
    int sockfd = socket(AF_UNIX, SOCK_STREAM, 0);

    if (sockfd == -1) {
        fprintf(stderr, "could not create socket: %s\n", strerror(errno));
        return -1;
    }

    struct sockaddr_un saddr = {AF_UNIX, DEFAULT_SOCK_PATH};

    int err = connect(sockfd, (struct sockaddr*)&saddr, sizeof(saddr));

    if (err) {
        fprintf(stderr, "could not connect socket: %s\n", strerror(errno));
        return -1;
    }

    struct pollfd pfd = {
        .fd = fd,
        .events = POLLIN,
    };

    while (!err) {
        if (poll(&pfd, 1, -1) != 1) {
            fprintf(stderr, "poll returned error: %s\n", strerror(errno));
            return -1;
        }

        if (pfd.revents & POLLIN) {
            int val = swm_read(fd);

            if (val == -1) {
                err |= -1;
            }

            if (val > 0) {
                err |= swm_write(sockfd, val);
            }
        }

        if (pfd.revents & POLLERR) {
            fprintf(stderr, "received POLLERR");
            err |= -1;
        }

        if (pfd.revents & POLLHUP) {
            fprintf(stderr, "received POLLHUP");
            err |= -1;
        }

        if (pfd.revents & POLLNVAL) {
            fprintf(stderr, "received POLLNVAL");
            err |= -1;
        }
    }

    return err;
}

static int swm_quit(int fd) { return close(fd); }
