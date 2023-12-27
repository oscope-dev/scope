#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#define DYLD_INTERPOSE(_replacement,_replacee) \
   __attribute__((used)) static struct{ const void* replacement; const void* replacee; } _interpose_##_replacee \
            __attribute__ ((section ("__DATA,__interpose"))) = { (const void*)(unsigned long)&_replacement, (const void*)(unsigned long)&_replacee };

int wrapped_execve(const char *pathname, char *const _Nullable argv[], char *const _Nullable envp[]) {
    const char* replaced_path = getenv("PITY_REPORT_CONTAINER_PATH");
    if (replaced_path == NULL) {
        printf("Unable to wrapping call to %s\n", pathname);
        return execve(pathname, argv, envp);
    } else {
        printf("Wrapping call to %s with %s\n", pathname, replaced_path);
        return execve(replaced_path, argv, envp);
    }
}

DYLD_INTERPOSE(wrapped_execve, execve);