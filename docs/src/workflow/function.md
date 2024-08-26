# How does a workflow work?

A workflow is a set of actions that are executed in a specific order. It is structured as follows:

```yaml
# workflows/example.yaml

properties:
# Name, description, and author of the workflow

launch_conditions:
# Conditions that must be met for the workflow to be executed

actions:
# Definition and configuration of the actions that are used in the next section

workflow:
# The order in which the actions are executed, error handling and timeouts

reporting:
# Use of ZIP compression, encryption, and metadata collection
```


![how_it_works](../assets/how_it_works.png "flowchart of how the collector works" =400x)