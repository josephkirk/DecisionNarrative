using System.Runtime.InteropServices;
using System.Text.Json;

static class StoryletAbi
{
    private const string Library = "storylet_engine.dll";

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr dn_run_start([MarshalAs(UnmanagedType.LPUTF8Str)] string content,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string start, ulong seed, out IntPtr error);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr dn_run_view(IntPtr run, ushort participant);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr dn_run_act(IntPtr run, ushort participant, [MarshalAs(UnmanagedType.LPUTF8Str)] string action);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern int dn_run_rewind(IntPtr run, uint step);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void dn_run_free(IntPtr run);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void dn_string_free(IntPtr value);
}

static class Program
{
    private static JsonDocument ReadAndFree(IntPtr value)
    {
        if (value == IntPtr.Zero) throw new InvalidOperationException("Engine returned a null string.");
        try { return JsonDocument.Parse(Marshal.PtrToStringUTF8(value)!); }
        finally { StoryletAbi.dn_string_free(value); }
    }

    public static async Task Main(string[] args)
    {
        var path = args.Length > 0 ? args[0] : Path.Combine(AppContext.BaseDirectory, "run-content.json");
        var content = await File.ReadAllTextAsync(path);
        var run = StoryletAbi.dn_run_start(content, "{\"kind\":\"beginning\"}", 7, out var error);
        if (run == IntPtr.Zero)
        {
            using var detail = ReadAndFree(error);
            throw new InvalidOperationException(detail.RootElement.ToString());
        }
        try
        {
            using var first = ReadAndFree(StoryletAbi.dn_run_view(run, 0));
            var action = first.RootElement.GetProperty("scene").GetProperty("choices")[0].GetProperty("action").GetRawText();
            using var acted = ReadAndFree(StoryletAbi.dn_run_act(run, 0, action));
            if (acted.RootElement.TryGetProperty("error", out var failure)) throw new InvalidOperationException(failure.ToString());
            if (acted.RootElement.GetProperty("currentStep").GetInt32() != 1) throw new InvalidOperationException("Action did not advance the run.");
            if (StoryletAbi.dn_run_rewind(run, 0) != 0) throw new InvalidOperationException("Rewind failed.");
            using var rewound = ReadAndFree(StoryletAbi.dn_run_view(run, 0));
            if (rewound.RootElement.GetProperty("currentStep").GetInt32() != 0) throw new InvalidOperationException("Rewind did not restore the start.");
            Console.WriteLine("RunKernel start, act, view, and rewind passed through the C ABI.");
        }
        finally { StoryletAbi.dn_run_free(run); }
    }
}
