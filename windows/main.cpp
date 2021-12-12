#include <windows.h>
#include <tlhelp32.h>
#include <vector>
#include <string>
#include <iostream>

#define ERROR_AND_EXIT(error, ...) { char buffer[128]; sprintf(buffer, error, __VA_ARGS__); MessageBoxA(NULL, buffer, "Launcher", MB_OK | MB_ICONERROR); exit(1); }

BOOL inject_DLL(const char* file_name, int PID)
{
    HANDLE hProcess = OpenProcess(PROCESS_ALL_ACCESS, FALSE, PID);

    char DllPath[_MAX_PATH];
    if (GetFullPathName(file_name, _MAX_PATH, DllPath, NULL) == 0)
        ERROR_AND_EXIT("Unable to get the full path to client.dll");

    LPVOID LoadLibAddr = (LPVOID)GetProcAddress(GetModuleHandleA("kernel32.dll"), "LoadLibraryA");
    if (!LoadLibAddr)
        ERROR_AND_EXIT("Could note locate real address of LoadLibraryA! (%#x)", GetLastError());

    LPVOID pDllPath = VirtualAllocEx(hProcess, 0, strlen(DllPath), MEM_COMMIT, PAGE_READWRITE);
    if (!pDllPath)
        ERROR_AND_EXIT("Could not allocate Memory in target process! (%#x)", GetLastError());

    if (!WriteProcessMemory(hProcess, pDllPath, (LPVOID)DllPath, strlen(DllPath), NULL))
        ERROR_AND_EXIT("Could not write into the allocated memory! (%#x)", GetLastError());

    HANDLE hThread = CreateRemoteThread(hProcess, NULL, NULL, (LPTHREAD_START_ROUTINE)LoadLibAddr, pDllPath, 0, NULL);
    if (!hThread)
        ERROR_AND_EXIT("Could not open Thread with CreatRemoteThread API! (%#x)", GetLastError());

    WaitForSingleObject(hThread, INFINITE);

    if (VirtualFreeEx(hProcess, pDllPath, 0, MEM_RELEASE)) {
        //VirtualFreeEx(hProc, reinterpret_cast<int*>(pDllPath) + 0X010000, 0, MEM_RELEASE);
        printf("Memory was freed in target process\n");
    }

    CloseHandle(hThread);

    return true;
}

INT WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, PSTR lpCmdLine, INT nCmdShow) {
    auto ret = (int)ShellExecute(0, "open", "subrosa.exe", 0, 0, SW_SHOWNORMAL);
    if (ret <= 32) {
        DWORD dw = GetLastError();
        char szMsg[250];
        FormatMessage(
            FORMAT_MESSAGE_FROM_SYSTEM,
            0, dw, 0,
            szMsg, sizeof(szMsg),
            NULL
        );
        ERROR_AND_EXIT(szMsg);
        return 1;
    }
    std::vector<DWORD> pids;
    std::wstring targetProcessName = L"subrosa.exe";

    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0); //all processes

    PROCESSENTRY32W entry; //current process
    entry.dwSize = sizeof entry;

    if (!Process32FirstW(snap, &entry)) { //start with the first in snapshot
        return 0;
    }

    do {
        if (std::wstring(entry.szExeFile) == targetProcessName) {
            pids.emplace_back(entry.th32ProcessID); //name matches; add to list
        }
    } while (Process32NextW(snap, &entry)); //keep going until end of snapshot

    if (pids.size() > 1)
        ERROR_AND_EXIT("More than one process found!");
    if (pids.size() <= 0)
        ERROR_AND_EXIT("No process found!");

    int PID = pids[0];

    if (!inject_DLL("client.dll", PID))
        ERROR_AND_EXIT("Unable to inject client!");

    return 0;
}