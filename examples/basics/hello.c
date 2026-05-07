#include <stdio.h>
#include <string.h>

int main(void) {
    char buf[16];
    memset(buf, 0, sizeof buf);
    memcpy(buf, "world", 5);
    printf("hello, %s! %d\n", buf, 42);
    puts("done");
    return 0;
}
