#pragma once
#include <filesystem>
#include <stdexcept>
#ifdef _WIN32
#define NOMINMAX
#include <windows.h>
#else
#include <sys/file.h>
#include <fcntl.h>
#include <unistd.h>
#endif

// Shared with the Rust owner, so recovery cannot reclaim a stage while either
// process still uses it. Acquisition precedes all reads/writes in the stage.
class StageLease {
#ifdef _WIN32
    HANDLE handle=INVALID_HANDLE_VALUE;
#else
    int handle=-1;
#endif
public:
    explicit StageLease(std::filesystem::path stage) {
        stage.replace_extension(".lease");
#ifdef _WIN32
        handle=CreateFileW(stage.c_str(),GENERIC_READ|GENERIC_WRITE,
            FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,nullptr,OPEN_EXISTING,0,nullptr);
        OVERLAPPED offset{};
        if(handle==INVALID_HANDLE_VALUE || !LockFileEx(handle,LOCKFILE_FAIL_IMMEDIATELY,0,MAXDWORD,MAXDWORD,&offset)) {
            if(handle!=INVALID_HANDLE_VALUE)CloseHandle(handle);
            throw std::runtime_error("cannot acquire stage lease");
        }
#else
        handle=open(stage.c_str(),O_RDWR|O_CLOEXEC);
        if(handle<0 || flock(handle,LOCK_SH|LOCK_NB)!=0) {
            if(handle>=0)close(handle);
            throw std::runtime_error("cannot acquire stage lease");
        }
#endif
    }
    StageLease(const StageLease&)=delete;
    StageLease& operator=(const StageLease&)=delete;
    ~StageLease() {
#ifdef _WIN32
        CloseHandle(handle);
#else
        close(handle);
#endif
    }
};
