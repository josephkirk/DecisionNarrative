using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

[StructLayout(LayoutKind.Sequential)]
struct DnBuffer
{
    public IntPtr Data;
    public nuint Len;
}

static class StoryletAbi
{
    private const string Library = "storylet_engine.dll";

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern uint dn_engine_abi_version();

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr dn_playtest_create();

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void dn_playtest_destroy(IntPtr handle);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern DnBuffer dn_playtest_start(IntPtr handle, byte[] projectJson, nuint projectLen, byte[] scenarioId, nuint scenarioLen);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern DnBuffer dn_playtest_evaluate(IntPtr handle, double targetTensionSlope);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern DnBuffer dn_playtest_execute_choice(IntPtr handle, byte[] choiceJson, nuint choiceLen);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern DnBuffer dn_playtest_snapshot(IntPtr handle);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void dn_buffer_free(DnBuffer buffer);
}

static class Program
{
    private static string ReadAndFree(DnBuffer buffer)
    {
        try
        {
            var length = checked((int)buffer.Len);
            var bytes = new byte[length];
            Marshal.Copy(buffer.Data, bytes, 0, length);
            return Encoding.UTF8.GetString(bytes);
        }
        finally
        {
            StoryletAbi.dn_buffer_free(buffer);
        }
    }

    private static JsonDocument RequireSuccess(string json)
    {
        var document = JsonDocument.Parse(json);
        if (!document.RootElement.GetProperty("ok").GetBoolean())
        {
            var error = document.RootElement.GetProperty("error").GetString();
            document.Dispose();
            throw new InvalidOperationException(error);
        }
        return document;
    }

    public static async Task Main()
    {
        if (StoryletAbi.dn_engine_abi_version() != 1)
            throw new InvalidOperationException("Unsupported storylet engine ABI version.");

        var handle = StoryletAbi.dn_playtest_create();
        if (handle == IntPtr.Zero) throw new InvalidOperationException("Could not create the storylet engine.");
        try
        {
            var project = await File.ReadAllBytesAsync(Path.Combine(AppContext.BaseDirectory, "storylet-project.json"));
            var scenario = Encoding.UTF8.GetBytes("opening");
            using (RequireSuccess(ReadAndFree(StoryletAbi.dn_playtest_start(handle, project, (nuint)project.Length, scenario, (nuint)scenario.Length))))
                Console.WriteLine("Started the generic Archive Mystery scenario through the C ABI.");

            using (var evaluation = RequireSuccess(ReadAndFree(StoryletAbi.dn_playtest_evaluate(handle, 0.15))))
            {
                var selected = evaluation.RootElement.GetProperty("result").GetProperty("selected").GetString();
                if (selected != "read-receipt") throw new InvalidOperationException($"Unexpected selection: {selected}");
                Console.WriteLine($"Rust director selected {selected}.");
            }

            var choice = Encoding.UTF8.GetBytes("{\"storyletId\":\"read-receipt\",\"choiceId\":\"read\",\"idempotencyKey\":\"dotnet:read-receipt\",\"expectedRevision\":0}");
            using (var applied = RequireSuccess(ReadAndFree(StoryletAbi.dn_playtest_execute_choice(handle, choice, (nuint)choice.Length))))
            {
                var clue = applied.RootElement.GetProperty("result").GetProperty("world").GetProperty("qualities").GetProperty("clue.receipt").GetBoolean();
                if (!clue) throw new InvalidOperationException("Storylet effect did not reach the native host.");
                Console.WriteLine("Choice effect reached the native host.");
            }

            using var snapshot = RequireSuccess(ReadAndFree(StoryletAbi.dn_playtest_snapshot(handle)));
            Console.WriteLine(snapshot.RootElement.GetProperty("result").GetProperty("world").GetRawText());
        }
        finally
        {
            StoryletAbi.dn_playtest_destroy(handle);
        }
    }
}
