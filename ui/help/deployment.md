# Deployment views

The logical model says what your system *is*. A deployment view says where it
actually **runs** — machines, runners, images, and the containers living on
them. Press **D** in the level bar, or dive from the Deployment section of the
tree.

## The shape

Deployment lives in one file, `model/deployment.yaml`:

```yaml
environments:
  production:
    name: Production
    nodes:
      eu-west:
        name: EU West
        tech: AWS
        nodes:              # nest as deep as needed
          app-server:
            name: App Server
            instances:
              api: { container: shop.api }
              web: { container: shop.web }
      cdn:
        name: CDN
    relations:
      - from: cdn
        to: eu-west
        label: origin
```

Three kinds:

- **Environment** — the top of one deployment tree. Production, staging, a
  developer's laptop, CI. Model as many as you have.
- **Deployment node** — anything a thing runs *on*: a region, a host, a
  container image, a runtime. They nest arbitrarily.
- **Container instance** — a container from your logical model, actually
  running somewhere. `container:` names it.

## It dives, it does not nest

C4 conventionally draws deployment as boxes inside boxes. Blastradius draws it
the way it draws everything else: **one altitude at a time**. The overview
lists your environments; dive into one to see its nodes, dive again to reach
what runs there.

That keeps one navigation model across the whole product — what you learned
flying through containers and components works unchanged here.

## Instances are checked

`container:` is a real reference. Point it at something that does not exist, or
at a system instead of a container, and validation fails — so a deployment
cannot quietly drift out of date when a container is renamed or removed. That
is the reason to model this rather than describe it in prose.

An instance shows the name of the container it runs, so the deployment view
speaks the same language as the rest of the model. Give it its own `name:` when
you need to distinguish two instances of the same thing.

## Everything else works as usual

Deployment elements are ordinary elements. They take relations (including to
and from logical elements), layout pins, document links, and canvas editing;
they appear in diffs, in exports, and to [coding agents](agents.md) exactly
like containers and components do.
