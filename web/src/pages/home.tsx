import { Layout } from '../layout.js';
import { createStream, type Stream } from '../stream.js';


export async function handleHomePage() {
  let stream = await createStream('http://localhost:8080/hosts/events');

  return {
    contents: (
      <Layout activeNavigationEntryId="servers">
        <HomePage useHostsStream={stream} />
      </Layout>
    ),
    title: 'Home',
  };
}

export function HomePage(props: {
  useHostsStream: Stream<{}>,
}) {
  let state = props.useHostsStream();

  return (
    <div>
      <h1>Home</h1>
      <pre>{JSON.stringify(state, null, 2)}</pre>
    </div>
  );
}
