using System.Buffers.Binary;
using System.IO.Pipes;
using System.Text;
using DeckyMyRigHostControl.Core;

namespace DeckyMyRigHostControl.Core.Tests;

[TestClass]
public sealed class NamedPipeManagementClientTests
{
    [TestMethod]
    public async Task GenerateCodeUsesFramedJsonOverARealLocalPipe()
    {
        var pipeName = $"DeckyMyRigHostControl-test-{Guid.NewGuid():N}";
        await using var server = new NamedPipeServerStream(
            pipeName,
            PipeDirection.InOut,
            1,
            PipeTransmissionMode.Byte,
            PipeOptions.Asynchronous);
        var serverTask = Task.Run(async () =>
        {
            await server.WaitForConnectionAsync();
            var requestLengthBytes = new byte[4];
            await server.ReadExactlyAsync(requestLengthBytes);
            var requestLength = BinaryPrimitives.ReadUInt32LittleEndian(requestLengthBytes);
            var request = new byte[checked((int)requestLength)];
            await server.ReadExactlyAsync(request);
            StringAssert.Contains(Encoding.UTF8.GetString(request), "generate_pairing_code");

            var response = Encoding.UTF8.GetBytes("{\"ok\":true,\"error\":null,\"serviceRunning\":true,\"port\":48100,\"paired\":false,\"pairingCode\":\"483921\",\"expiresInSeconds\":272}");
            var responseLength = new byte[4];
            BinaryPrimitives.WriteUInt32LittleEndian(responseLength, (uint)response.Length);
            await server.WriteAsync(responseLength);
            await server.WriteAsync(response);
            await server.FlushAsync();
        });

        var state = await new NamedPipeManagementClient(pipeName).GeneratePairingCodeAsync();
        await serverTask;

        Assert.IsTrue(state.Ok);
        Assert.IsTrue(state.ServiceRunning);
        Assert.AreEqual(48100, state.Port);
        Assert.AreEqual("483921", state.PairingCode);
        Assert.AreEqual(272UL, state.ExpiresInSeconds);
    }
}
