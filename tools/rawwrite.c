/*
 * rawwrite.c - Write raw image to physical disk (Windows)
 * Compile: gcc -o rawwrite rawwrite.c
 * Usage: rawwrite <image_file> <disk_number>
 * Example: rawwrite BOOT.FBC 1
 */

#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

int main(int argc, char **argv) {
    if (argc != 3) {
        printf("Usage: %s <image_file> <disk_number>\n", argv[0]);
        printf("Example: %s BOOT.FBC 1\n", argv[0]);
        return 1;
    }

    const char *image_path = argv[1];
    int disk_num = atoi(argv[2]);
    char disk_path[64];
    snprintf(disk_path, sizeof(disk_path), "\\\\.\\PhysicalDrive%d", disk_num);

    // Open image file
    FILE *img = fopen(image_path, "rb");
    if (!img) {
        printf("ERROR: Cannot open image file: %s\n", image_path);
        return 1;
    }

    // Get image size
    fseek(img, 0, SEEK_END);
    long img_size = ftell(img);
    fseek(img, 0, SEEK_SET);
    printf("Image: %s (%ld bytes)\n", image_path, img_size);

    // Open physical disk
    HANDLE disk = CreateFileA(
        disk_path,
        GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        NULL,
        OPEN_EXISTING,
        0,
        NULL
    );

    if (disk == INVALID_HANDLE_VALUE) {
        printf("ERROR: Cannot open %s (error %lu)\n", disk_path, GetLastError());
        printf("Make sure you run as Administrator!\n");
        fclose(img);
        return 1;
    }

    printf("Writing to %s...\n", disk_path);

    // Write in 512-byte sectors
    unsigned char buf[512];
    DWORD written;
    long total = 0;
    int sectors = 0;

    while (1) {
        size_t n = fread(buf, 1, 512, img);
        if (n == 0) break;

        // Pad last sector with zeros if needed
        if (n < 512) memset(buf + n, 0, 512 - n);

        if (!WriteFile(disk, buf, 512, &written, NULL) || written != 512) {
            printf("ERROR: Write failed at sector %d (error %lu)\n", sectors, GetLastError());
            CloseHandle(disk);
            fclose(img);
            return 1;
        }

        total += n;
        sectors++;

        if (sectors % 1000 == 0) {
            printf("  %d sectors (%ld bytes)...\n", sectors, total);
        }
    }

    // Flush
    FlushFileBuffers(disk);

    printf("Done! Wrote %d sectors (%ld bytes)\n", sectors, total);

    CloseHandle(disk);
    fclose(img);
    return 0;
}
