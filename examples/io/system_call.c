// system() bridges to gawk's blocking system() builtin (which forks
// /bin/sh -c "..." and waits). Returns the child exit status.
//
// Test against `true` and `false` POSIX utilities so we don't depend
// on stdout interleaving — just the return code is asserted, which
// is what nearly every real `system()` call cares about. Pipe-style
// "capture child output" is what popen exists for, covered separately.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    int rc_true  = system("true");
    int rc_false = system("false");
    printf("true=%d false=%d\n", rc_true == 0, rc_false != 0);
    return 0;
}
