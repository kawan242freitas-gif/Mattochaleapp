param(
    [Parameter(Mandatory = $true)][string]$ExecutablePath,
    [Parameter(Mandatory = $true)][string]$IconPath
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $ExecutablePath)) { throw "Executável não encontrado: $ExecutablePath" }
if (-not (Test-Path -LiteralPath $IconPath)) { throw "Ícone não encontrado: $IconPath" }

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;

public static class MattoExecutableIcon
{
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr BeginUpdateResourceW(string fileName, bool deleteExistingResources);

    [DllImport("kernel32.dll", EntryPoint = "UpdateResourceW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool UpdateResource(IntPtr update, IntPtr type, IntPtr name, ushort language, byte[] data, uint dataSize);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool EndUpdateResourceW(IntPtr update, bool discard);

    private static IntPtr Id(int value) { return new IntPtr(value); }

    public static void Apply(string executablePath, string iconPath)
    {
        byte[] ico = File.ReadAllBytes(iconPath);
        if (ico.Length < 6 || BitConverter.ToUInt16(ico, 0) != 0 || BitConverter.ToUInt16(ico, 2) != 1)
            throw new InvalidDataException("O arquivo não é um ícone ICO válido.");
        int count = BitConverter.ToUInt16(ico, 4);
        if (count < 1 || ico.Length < 6 + count * 16) throw new InvalidDataException("O arquivo ICO está incompleto.");

        IntPtr update = BeginUpdateResourceW(executablePath, false);
        if (update == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "Não foi possível abrir o executável para atualizar o ícone.");
        bool committed = false;
        try
        {
            using (var group = new MemoryStream())
            using (var writer = new BinaryWriter(group))
            {
                writer.Write((ushort)0);
                writer.Write((ushort)1);
                writer.Write((ushort)count);
                for (int i = 0; i < count; i++)
                {
                    int entry = 6 + 16 * i;
                    int bytes = checked((int)BitConverter.ToUInt32(ico, entry + 8));
                    int offset = checked((int)BitConverter.ToUInt32(ico, entry + 12));
                    if (offset < 0 || bytes < 1 || offset + bytes > ico.Length) throw new InvalidDataException("Uma imagem do ICO está fora do arquivo.");
                    ushort resourceId = (ushort)(i + 1);
                    byte[] imageData = new byte[bytes];
                    Buffer.BlockCopy(ico, offset, imageData, 0, bytes);
                    if (!UpdateResource(update, Id(3), Id(resourceId), 0, imageData, (uint)imageData.Length))
                        throw new Win32Exception(Marshal.GetLastWin32Error(), "Não foi possível gravar uma imagem do ícone.");

                    writer.Write(ico[entry]);
                    writer.Write(ico[entry + 1]);
                    writer.Write(ico[entry + 2]);
                    writer.Write(ico[entry + 3]);
                    writer.Write(BitConverter.ToUInt16(ico, entry + 4));
                    writer.Write(BitConverter.ToUInt16(ico, entry + 6));
                    writer.Write((uint)bytes);
                    writer.Write(resourceId);
                }
                byte[] groupData = group.ToArray();
                if (!UpdateResource(update, Id(14), Id(1), 0, groupData, (uint)groupData.Length))
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "Não foi possível gravar o grupo do ícone.");
            }
            if (!EndUpdateResourceW(update, false)) throw new Win32Exception(Marshal.GetLastWin32Error(), "Não foi possível finalizar a atualização do ícone.");
            committed = true;
        }
        finally
        {
            if (!committed) EndUpdateResourceW(update, true);
        }
    }
}
'@

[MattoExecutableIcon]::Apply((Resolve-Path -LiteralPath $ExecutablePath).Path, (Resolve-Path -LiteralPath $IconPath).Path)
Write-Output "Ícone nativo aplicado: $ExecutablePath"
