# The reference machine as a night's record names it (piece G107; chapter 28.17 and decision 32.16
# of the kernel architecture). Windows PowerShell 5.1, run by nightly.sh through WSL's interop.
#
# Prints four lines of the record's header, and nothing that names the machine itself:
#
#   machine <the processor's name>
#   processors <logical processors>
#   performance <the performance cores' logical processor numbers, increasing>
#   os <the operating system's name and version>
#
# The performance cores are the logical processors of the highest efficiency class Windows reports
# (GetSystemCpuSetInformation, SYSTEM_CPU_SET_INFORMATION.CpuSet.EfficiencyClass: "a higher value
# indicates a more performant core", https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-system_cpu_set_information).
# On a processor with one kind of core every logical processor is listed. The structure is read by
# its documented offsets: Size at 0, Group at 12, LogicalProcessorIndex at 14, EfficiencyClass at 18.
# Only processor group 0 is read; a machine with more than 64 logical processors stops with an
# error rather than report part of itself.

$ErrorActionPreference = 'Stop'

Add-Type -Namespace SpareKit -Name CpuSets -MemberDefinition @'
[DllImport("kernel32.dll", SetLastError = true)]
public static extern bool GetSystemCpuSetInformation(System.IntPtr info, uint length, out uint returned, System.IntPtr process, uint flags);
'@

$length = [uint32]0
[void][SpareKit.CpuSets]::GetSystemCpuSetInformation([IntPtr]::Zero, 0, [ref]$length, [IntPtr]::Zero, 0)
$buffer = [Runtime.InteropServices.Marshal]::AllocHGlobal([int]$length)
try {
    if (-not [SpareKit.CpuSets]::GetSystemCpuSetInformation($buffer, $length, [ref]$length, [IntPtr]::Zero, 0)) {
        throw "GetSystemCpuSetInformation failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    }
    $sets = @()
    $offset = 0
    while ($offset -lt $length) {
        $size = [Runtime.InteropServices.Marshal]::ReadInt32($buffer, $offset)
        if ($size -le 0) { throw "a CPU set entry of size $size" }
        $sets += [pscustomobject]@{
            Group = [Runtime.InteropServices.Marshal]::ReadInt16($buffer, $offset + 12)
            Index = [Runtime.InteropServices.Marshal]::ReadByte($buffer, $offset + 14)
            Class = [Runtime.InteropServices.Marshal]::ReadByte($buffer, $offset + 18)
        }
        $offset += $size
    }
} finally {
    [Runtime.InteropServices.Marshal]::FreeHGlobal($buffer)
}

if (@($sets | Where-Object { $_.Group -ne 0 }).Count -gt 0) {
    throw 'more than one processor group: this script reads group 0 only'
}
$best = ($sets | Measure-Object -Property Class -Maximum).Maximum
$performance = @($sets | Where-Object { $_.Class -eq $best } | ForEach-Object { [int]$_.Index } | Sort-Object)

$processor = @(Get-CimInstance -ClassName Win32_Processor)[0]
$os = Get-CimInstance -ClassName Win32_OperatingSystem

"machine $(($processor.Name -replace '\s+', ' ').Trim())"
"processors $($sets.Count)"
"performance $($performance -join ' ')"
"os $($os.Caption.Trim()) $($os.Version)"
