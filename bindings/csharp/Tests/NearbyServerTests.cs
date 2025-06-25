using InterShareSdk;

namespace Tests;

public class NearbyServerTests : NearbyConnectionDelegate
{
    [Test]
    public async Task Setup()
    {
        var device = new Device(id: "97658e56-dc41-4ff2-a2f4-876dac4a5d30", name: "Windows PC Test", deviceType: 0);

        var discovery = new NearbyServer(device, this);
        await discovery.Start();

        while (true)
        {
            await Task.Delay(100);
        }
    }

    public void ReceivedConnectionRequest(ConnectionRequest request)
    {
        Assert.Pass();
    }
}
