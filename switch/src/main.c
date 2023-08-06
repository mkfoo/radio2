#include "switchmon.h"
#include <stdlib.h>

int main(void) {
    int fd = swm_init();

    if (fd == -1) {
        return EXIT_FAILURE;
    }

    int err = swm_poll(fd);

    if (err) {
        return EXIT_FAILURE;
    }

    swm_quit(fd);
    return EXIT_SUCCESS;
}
